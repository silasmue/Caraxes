#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""pporl: PPO on a vectorized CartPole, the operator's RL benchmark (2026-09-24).

The environment runs on the CPU; the policy and the PPO update run on the GPU. Every
simulation step makes one small host-to-device transfer (the observations), a handful of
kernel launches (the actor-critic forward pass) and one synchronous device-to-host transfer
(the actions), which is the RL pattern the Caraxes submission and copy paths are measured
against. Output: steps per second and the time to a mean-100 return of 475 ("solved"), plus
one machine-readable line `# PPO steps_per_s=... solve_s=... episodes=...` for the harness.

Usage: pporl.py [--seconds 420] [--envs 128] [--rollout 256] [--epochs 4] [--batch 4096]
"""

import argparse
import math
import time
from collections import deque

import numpy as np
import torch
import torch.nn as nn
from torch.distributions import Categorical


class VecCartPole:
    """Vectorized CartPole with the classic control equations; runs on the CPU."""

    def __init__(self, n_envs=128, seed=1):
        self.n = n_envs
        self.rng = np.random.default_rng(seed)
        self.gravity = 9.8
        self.masscart = 1.0
        self.masspole = 0.1
        self.total_mass = self.masscart + self.masspole
        self.length = 0.5
        self.polemass_length = self.masspole * self.length
        self.force_mag = 10.0
        self.tau = 0.02
        self.theta_threshold = 12 * 2 * math.pi / 360
        self.x_threshold = 2.4
        self.max_steps = 500
        self.state = np.zeros((self.n, 4), dtype=np.float32)
        self.ep_return = np.zeros(self.n, dtype=np.float32)
        self.ep_length = np.zeros(self.n, dtype=np.int32)
        self.reset(np.arange(self.n))

    def reset(self, indices):
        count = len(indices)
        self.state[indices] = self.rng.uniform(low=-0.05, high=0.05, size=(count, 4)).astype(np.float32)
        self.ep_return[indices] = 0.0
        self.ep_length[indices] = 0

    def step(self, actions):
        x = self.state[:, 0]
        x_dot = self.state[:, 1]
        theta = self.state[:, 2]
        theta_dot = self.state[:, 3]
        force = np.where(actions == 1, self.force_mag, -self.force_mag)
        costheta = np.cos(theta)
        sintheta = np.sin(theta)
        temp = (force + self.polemass_length * theta_dot**2 * sintheta) / self.total_mass
        thetaacc = (self.gravity * sintheta - costheta * temp) / (
            self.length * (4.0 / 3.0 - self.masspole * costheta**2 / self.total_mass)
        )
        xacc = temp - self.polemass_length * thetaacc * costheta / self.total_mass
        x = x + self.tau * x_dot
        x_dot = x_dot + self.tau * xacc
        theta = theta + self.tau * theta_dot
        theta_dot = theta_dot + self.tau * thetaacc
        self.state[:, 0] = x
        self.state[:, 1] = x_dot
        self.state[:, 2] = theta
        self.state[:, 3] = theta_dot
        self.ep_length += 1
        terminated = (
            (x < -self.x_threshold)
            | (x > self.x_threshold)
            | (theta < -self.theta_threshold)
            | (theta > self.theta_threshold)
            | (self.ep_length >= self.max_steps)
        )
        reward = np.ones(self.n, dtype=np.float32)
        self.ep_return += reward
        completed_returns = self.ep_return[terminated].copy()
        completed_lengths = self.ep_length[terminated].copy()
        if np.any(terminated):
            self.reset(np.flatnonzero(terminated))
        return (self.state.copy(), reward, terminated.astype(np.float32), completed_returns, completed_lengths)


class ActorCritic(nn.Module):
    def __init__(self):
        super().__init__()
        self.body = nn.Sequential(nn.Linear(4, 128), nn.Tanh(), nn.Linear(128, 128), nn.Tanh())
        self.actor = nn.Linear(128, 2)
        self.critic = nn.Linear(128, 1)

    def forward(self, x):
        h = self.body(x)
        return self.actor(h), self.critic(h).squeeze(-1)

    def act(self, x):
        logits, value = self(x)
        dist = Categorical(logits=logits)
        action = dist.sample()
        return action, dist.log_prob(action), value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--seconds", type=int, default=420)
    parser.add_argument("--envs", type=int, default=128)
    parser.add_argument("--rollout", type=int, default=256)
    parser.add_argument("--epochs", type=int, default=4)
    parser.add_argument("--batch", type=int, default=4096)
    parser.add_argument("--seed", type=int, default=1)
    args = parser.parse_args()

    np.random.seed(args.seed)
    torch.manual_seed(args.seed)
    if not torch.cuda.is_available():
        raise RuntimeError("CUDA GPU required")
    device = torch.device("cuda")
    print("device:", torch.cuda.get_device_name())
    print("parallel environments:", args.envs)
    print("runtime:", args.seconds, "seconds")

    env = VecCartPole(args.envs, args.seed)
    model = ActorCritic().to(device)
    optimizer = torch.optim.Adam(model.parameters(), lr=3e-4, eps=1e-5)
    gamma, gae_lambda, clip_eps, value_coef, entropy_coef = 0.99, 0.95, 0.2, 0.5, 0.01
    T, N = args.rollout, args.envs

    obs_buf = torch.empty((T, N, 4), device=device)
    action_buf = torch.empty((T, N), dtype=torch.long, device=device)
    logp_buf = torch.empty((T, N), device=device)
    value_buf = torch.empty((T, N), device=device)
    reward_cpu = np.empty((T, N), dtype=np.float32)
    done_cpu = np.empty((T, N), dtype=np.float32)
    episode_history = deque(maxlen=100)
    obs = env.state.copy()
    total_steps = 0
    total_episodes = 0
    solved_at = None
    start = time.perf_counter()
    last_print = start

    while True:
        elapsed = time.perf_counter() - start
        if elapsed >= args.seconds:
            break
        # Rollout: one small H2D and one synchronous D2H transfer per simulation step.
        for t in range(T):
            obs_gpu = torch.from_numpy(obs).to(device, non_blocking=False)
            with torch.no_grad():
                action, logp, value = model.act(obs_gpu)
            obs_buf[t].copy_(obs_gpu)
            action_buf[t].copy_(action)
            logp_buf[t].copy_(logp)
            value_buf[t].copy_(value)
            action_cpu = action.cpu().numpy()
            obs, reward, done, returns, lengths = env.step(action_cpu)
            reward_cpu[t] = reward
            done_cpu[t] = done
            for ret in returns:
                episode_history.append(float(ret))
                total_episodes += 1
            total_steps += N
        with torch.no_grad():
            next_obs_gpu = torch.from_numpy(obs).to(device)
            _, next_value = model(next_obs_gpu)
        rewards = torch.from_numpy(reward_cpu).to(device)
        dones = torch.from_numpy(done_cpu).to(device)
        # GAE
        advantages = torch.zeros_like(rewards)
        last_gae = torch.zeros(N, device=device)
        for t in reversed(range(T)):
            next_values = next_value if t == T - 1 else value_buf[t + 1]
            next_nonterminal = 1.0 - dones[t]
            delta = rewards[t] + gamma * next_values * next_nonterminal - value_buf[t]
            last_gae = delta + gamma * gae_lambda * next_nonterminal * last_gae
            advantages[t] = last_gae
        returns = advantages + value_buf
        b_obs = obs_buf.reshape(-1, 4)
        b_actions = action_buf.reshape(-1)
        b_logp = logp_buf.reshape(-1)
        b_adv = advantages.reshape(-1)
        b_returns = returns.reshape(-1)
        b_adv = (b_adv - b_adv.mean()) / (b_adv.std() + 1e-8)
        batch_size = T * N
        indices = torch.arange(batch_size, device=device)
        # PPO update
        for _ in range(args.epochs):
            perm = indices[torch.randperm(batch_size, device=device)]
            for start_idx in range(0, batch_size, args.batch):
                idx = perm[start_idx:start_idx + args.batch]
                logits, values = model(b_obs[idx])
                dist = Categorical(logits=logits)
                new_logp = dist.log_prob(b_actions[idx])
                entropy = dist.entropy().mean()
                ratio = torch.exp(new_logp - b_logp[idx])
                pg1 = ratio * b_adv[idx]
                pg2 = torch.clamp(ratio, 1.0 - clip_eps, 1.0 + clip_eps) * b_adv[idx]
                policy_loss = -torch.min(pg1, pg2).mean()
                value_loss = ((values - b_returns[idx]) ** 2).mean()
                loss = policy_loss + value_coef * value_loss - entropy_coef * entropy
                optimizer.zero_grad(set_to_none=True)
                loss.backward()
                nn.utils.clip_grad_norm_(model.parameters(), 0.5)
                optimizer.step()
        now = time.perf_counter()
        if now - last_print >= 5:
            runtime = now - start
            steps_per_sec = total_steps / runtime
            mean100 = np.mean(episode_history) if episode_history else 0.0
            print(f"{runtime:7.1f}s | steps {total_steps:10d} | {steps_per_sec:9.0f} steps/s | "
                  f"episodes {total_episodes:7d} | reward100 {mean100:7.1f}")
            if mean100 >= 475 and solved_at is None:
                solved_at = runtime
                print(f"\n*** SOLVED after {solved_at:.1f} seconds ***\n")
            last_print = now

    torch.cuda.synchronize()
    runtime = time.perf_counter() - start
    mean100 = float(np.mean(episode_history)) if episode_history else 0.0
    print("\n=== final ===")
    print(f"runtime:       {runtime:.2f} s")
    print(f"steps:         {total_steps}")
    print(f"episodes:      {total_episodes}")
    print(f"steps/s:       {total_steps / runtime:.0f}")
    print(f"reward100:     {mean100:.1f}")
    print("solve time:    not solved" if solved_at is None else f"solve time:    {solved_at:.2f} s")
    print(f"# PPO steps_per_s={total_steps / runtime:.0f} solve_s={-1 if solved_at is None else round(solved_at, 2)} "
          f"episodes={total_episodes} reward100={mean100:.1f} envs={args.envs} rollout={args.rollout}")


if __name__ == "__main__":
    main()
