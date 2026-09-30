<!-- SPDX-License-Identifier: MIT -->
# Reporting a problem

One command collects what a bug report needs from a machine that runs Leandro VirtIO or
Leandro VFIO: host facts, the kernel log, the logs of each VM and the driver state of each
running guest, in one tarball. A card the project does not know yet needs a capture instead
([another card](#another-card)).

## Run

```sh
tools/leandro-report.sh --vm /mnt/vmstore/caraxes/vm0 --redact -o ~/leandro-report.tar.zst
```

- `--vm DIR`, once per VM: its state directory. Leandro VFIO: the launcher's
  `$CARAXES_STORE/$VM` (`gpu.log`, `ch.log`, `serial.log`, `gpu-trace.log`), or a Windows
  state directory, whose `current` names the run folder. Leandro VirtIO: the directory with
  `nvrm.log` or `backend.log`, `ch.log` or `hypervisor.log` and `serial.log`.
- `--guest user@address`, once per running guest, Linux or Windows (PowerShell over ssh). The
  key is `$LEA_VM_DIR/id_leandro` when present, else `--key FILE`.
- `--since "2 hours ago"`: the host kernel log window, in journalctl's syntax; widen it to
  cover the crash.
- `--redact` for anything posted in public.
- `--lines N` (default 300) and `--trace-mb N` (default 20) size the log tails and the trace
  tail.
- Run it right after the problem and before the next start: a start truncates `gpu.log`,
  `ch.log` and the trace.

The tool needs no root and changes nothing. It reads files, `/proc`, `/sys`, `lspci`,
`nvidia-smi -q` and the kernel log, and never writes to a GPU, a module or a parameter; under
sudo it runs the same steps and says so. Every step has a time limit; a step that fails or
times out is noted in the manifest and skipped. `tools/leandro-report.sh --self-test` checks
the collectors and the redaction against a fixture; `tools/check.sh` runs it.

## What it collects

| Part | Contents |
|---|---|
| `host/` | OS, kernel and its command line, CPU, RAM; `lspci -nnk`, `lspci -vv` of the NVIDIA functions and their IOMMU group; `/proc/driver/nvidia/version` and `params`, `nvidia-smi -q`; the Cloud Hypervisor and QEMU versions; the kernel log lines with NVRM, Xid, nvidia, vfio, kvm or ublk since `--since`; free memory and the free space of the VM store |
| `repo.txt`, `binaries.txt` | `git describe --always --dirty` of the checkout; the sha256 of every device binary a state directory names or runs |
| `vm-N/` | the device's `config:` block, its ERROR and WARNING lines and its exit summary; the tails of the device, hypervisor and serial logs; `env.json`, `start.json`, `launch.log`; the command lines of the VM's running processes; the host's Xid lines for the device's pid; the last 20 MB of `gpu-trace.log`, compressed |
| `guest-N/` | Linux: `uname -a`, the driver version and modules, `nvidia-smi -q`, the kernel log's NVRM and Xid lines. Windows: the display adapters and their driver versions, the display-driver, WER and LiveKernelEvent entries of the last 24 hours, the LiveKernelReports and WER report folders |
| `MANIFEST.txt` | one line per step (`ok`, `skipped`, `failed` or `timeout`, with a note), the redaction, the file list |

- Kernel log: `journalctl -k` where the user may read it, else `dmesg`; the manifest names the
  source. Without either (`kernel.dmesg_restrict=1` and no journal access) the step is
  skipped: run with sudo, or join the `systemd-journal` group.
- Next to the tarball: `<name>.tar.zst.sha256` and `<name>.manifest.txt`.

## Redaction

- Always: e-mail addresses become `<email>`, and a line naming a password, a passphrase or a
  Windows auto-logon value is dropped. The tool reads no registry and no credential store.
- With `--redact` also: the host name, the user's login and full name and the guests' host
  names become `<id>`; home directories become `/home/<user>` and `C:\Users\<user>`; IP and
  MAC addresses, UUIDs (GPU and file system), serial numbers and the names of other processes
  on the GPU are replaced.
- After the scrub a check searches every file, the trace included, for what was replaced, and
  writes no tarball while one remains. The tar entries carry no owner names.
- The tool cannot know every private string: unpack with `zstd -dc <name>.tar.zst | tar -xf -`
  and spot-check with `grep -ri` before posting.

## Where to send it

Attach the tarball and its `.sha256` to an issue in the project's GitHub repository, or send
both to the maintainers. Say in a few lines what you ran, what you expected and what
happened, and which guest and driver. A report without `--redact` carries host and user names
and addresses: send it privately.

## Another card

A card the project has not seen enters through a capture, not a report:
`caraxes/tools/lab-kit.sh` reads the card, its driver and its RM answers on a machine that
has the card, read-only, and leaves one redacted tarball
([contributing a capture](wiki/contributing-a-capture.md)).
