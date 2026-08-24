# Eclipse

Peer-to-peer remote desktop. Every install is a **symmetric node**: Tauri + React is the control plane; **Sunshine** (host) and **Moonlight-Qt** (client) are bundled data-plane sidecars. Discovery uses the OS Tailscale CLI, not a central relay.

**Status:** the app launches, starts a local Sunshine sidecar, lists other tailnet nodes (TCP probe to Sunshine `:47990`), and **Connect** pairs then opens Moonlight on Desktop. Native installers are not implemented yet.

| | Prepare the system | Install and run |
| --- | --- | --- |
| **Users** | GPU hardware encode/decode, Linux `input` group for injection, Tailscale when peers exist | No packages yet — build from source (same commands as developers) |
| **Developers** | Tauri, Rust, Node, Qt 6, CMake/Ninja, GPU drivers | Clone with submodules → `./scripts/setup-linux.sh` (or `.\scripts\setup-windows.ps1`) → `npm run tauri -- dev` |

- **[docs/User_Guide.md](docs/User_Guide.md)** — runtime requirements and how to run the app
- **[docs/Development.md](docs/Development.md)** — toolchain, submodules, sidecar build, troubleshooting
- **[docs/Architecture_Specification.md](docs/Architecture_Specification.md)** — product architecture (what we are building)

### Developer quick start

```bash
git clone --recurse-submodules <repo-url>
cd Eclipse
./scripts/setup-linux.sh        # Linux — real terminal: sudo / fingerprint
npm run tauri -- dev
```

Windows 11 (PowerShell; one UAC prompt):

```powershell
git clone --recurse-submodules <repo-url>
cd Eclipse
.\scripts\setup-windows.ps1
npm run tauri -- dev
```

`setup` installs the toolchain (including CUDA toolkit when an NVIDIA GPU is present), builds sidecars, and runs `npm install`. First run can take a while (VS Build Tools, Qt, and CUDA are large). Set `ECLIPSE_CUDA=off` to skip CUDA. Bootstrap does **not** install NVIDIA kernel / Game Ready drivers.

Moonlight is pinned to commit `d2f6990b`, not tag `v6.1.0` (that tag does not build against FFmpeg 8). Details in the development guide.
