#!/usr/bin/env python3
"""Link the real Caraxes backend with the paired VMM IRQ owner, without a VM."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--vmm", type=Path, required=True)
    parser.add_argument("--vmm-target", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="New result directory")
    args = parser.parse_args()
    backend, vmm, output = (p.resolve() for p in (args.backend, args.vmm, args.output))
    output.mkdir(parents=True, exist_ok=False)
    inputs = [
        backend / "Cargo.lock",
        backend / "vendor/vfio-user-rs/src/lib.rs",
        backend / "vendor/vfio-user-rs/src/local_irq.rs",
        backend / "crates/vfio-user-nvgpu/src/intr.rs",
        backend / "crates/vfio-user-nvgpu/src/intr_page.rs",
        backend / "crates/vfio-user-nvgpu/src/device.rs",
        backend / "crates/vfio-user-nvgpu/src/bar0.rs",
        backend / "crates/vfio-user-nvgpu/src/boot.rs",
        backend / "crates/vfio-user-nvgpu/src/mirror.rs",
        backend / "crates/vfio-user-nvgpu/src/display/engine.rs",
        backend / "tools/tests/local_irq_linked.rs",
        Path(__file__).resolve(),
        vmm / "Cargo.lock",
        vmm / "pci/src/caraxes_irq.rs",
        vmm / "pci/src/caraxes_irq_owner.rs",
        vmm / "pci/src/mmap.rs",
    ]
    before = {str(path): digest(path) for path in inputs}
    commands = []

    def run(name, argv, cwd, extra_env=None, timeout=300):
        start = time.time()
        env = os.environ.copy()
        env.update(extra_env or {})
        with (output / f"{name}.log").open("w") as log:
            result = subprocess.run(argv, cwd=cwd, env=env, stdout=log,
                                    stderr=subprocess.STDOUT, timeout=timeout)
        commands.append({"name": name, "argv": [str(x) for x in argv],
                         "cwd": str(cwd), "env": extra_env or {},
                         "seconds": time.time() - start, "exit_code": result.returncode})
        (output / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
        if result.returncode:
            raise RuntimeError(f"{name} failed; see {output / (name + '.log')}")
        return output / f"{name}.log"

    def artifacts(log):
        found = {}
        for line in log.read_text().splitlines():
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if message.get("reason") == "compiler-artifact":
                for filename in message.get("filenames", []):
                    if filename.endswith(".rlib"):
                        found[message["target"]["name"]] = filename
        return found

    backend_log = run("backend-build", ["cargo", "build", "--offline", "--locked",
                      "-p", "vfio-user-nvgpu", "--lib", "--message-format=json"], backend)
    vmm_log = run("allocator-build", ["cargo", "build", "--offline", "--locked",
                  "-p", "vm-allocator", "--message-format=json"], vmm,
                  {"CARGO_TARGET_DIR": str(args.vmm_target.resolve())})
    libs = artifacts(backend_log)
    libs["vm_allocator"] = artifacts(vmm_log)["vm_allocator"]
    wrapper = output / "linked.rs"
    wrapper.write_text("\n".join(
        f"#[path = {json.dumps(str(vmm / 'pci/src' / filename))}] mod {name};"
        for name, filename in [("caraxes_irq", "caraxes_irq.rs"),
                               ("caraxes_irq_owner", "caraxes_irq_owner.rs"),
                               ("mmap", "mmap.rs")]) +
        f"\ninclude!({json.dumps(str(backend / 'tools/tests/local_irq_linked.rs'))});\n")
    argv = ["rustc", "--edition=2024", "--test", "--check-cfg", "cfg(caraxes_reference)",
            "--check-cfg", "cfg(test)",
            str(wrapper), "-o", str(output / "linked-test")]
    for name in ["vfio_user_nvgpu", "vfio_user", "libc", "vmm_sys_util", "log", "vm_allocator"]:
        argv.extend(["--extern", f"{name}={libs[name]}"])
    directories = {str(Path(filename).parent) for filename in libs.values()}
    directories.update([str(backend / "target/debug/deps"),
                        str(args.vmm_target.resolve() / "debug/deps")])
    for directory in sorted(directories):
        argv.extend(["-L", f"dependency={directory}"])
    run("linked-build", argv, backend)
    run("linked-tests", [str(output / "linked-test"), "linked::", "--test-threads=1"],
        backend, timeout=60)
    after = {str(path): digest(path) for path in inputs}
    unchanged = before == after
    (output / "result.json").write_text(json.dumps({"passed": unchanged,
        "inputs_unchanged": unchanged, "inputs_before": before, "inputs_after": after,
        "libraries": {name: {"path": path, "sha256": digest(Path(path))}
                      for name, path in libs.items()},
        "hardware_used": False}, indent=2) + "\n")
    if not unchanged:
        raise RuntimeError("Source changed during verification; repeat with frozen inputs")
    print(f"PASS: linked backend/owner tests; no VM or GPU. Results: {output}")


if __name__ == "__main__":
    main()
