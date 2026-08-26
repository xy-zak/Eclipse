# Eclipse

Peer-to-peer remote desktop. Every install is a **symmetric node**: Tauri + React is the control plane; **Sunshine** (host) and **Moonlight-Qt** (client) are bundled data-plane sidecars. Discovery uses the OS Tailscale CLI, not a central relay.

**Status:** Packaged installs run Sunshine as a boot-time host service and an Eclipse tray after login (Host streaming on/off). **Connect** pairs then opens Moonlight on Desktop. Local native packages can be built with `./scripts/package.sh` (Linux) or `.\scripts\package.ps1` (Windows). GitHub Actions installers are not wired yet.

| | Prepare the system | Install and run |
| --- | --- | --- |
| **Users** | GPU hardware encode/decode, Linux `input` group for injection, Tailscale when peers exist | Build a package on that OS (`package.sh` / `package.ps1`) and install it, or run from source |
| **Developers** | Tauri, Rust, Node, Qt 6, CMake/Ninja, GPU drivers | Clone with submodules → `./scripts/setup-linux.sh` (or `.\scripts\setup-windows.ps1`) → `npm run tauri -- dev` |

- **[docs/User_Guide.md](docs/User_Guide.md)** — runtime requirements and how to run the app
- **[docs/Development.md](docs/Development.md)** — toolchain, submodules, sidecar build, troubleshooting
- **[docs/Architecture_Specification.md](docs/Architecture_Specification.md)** — product architecture (what we are building)
- **[patches/README.md](patches/README.md)** — Sunshine/Moonlight patch series and how to rebase them

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

`setup` installs the toolchain (including CUDA toolkit when an NVIDIA GPU is present), builds sidecars, and runs `npm install`. First run can take a while (VS Build Tools, Qt, and CUDA are large). Set `ECLIPSE_CUDA=off` to skip CUDA. Bootstrap does **not** install NVIDIA kernel / Game Ready drivers. The CUDA toolkit is a compile-time tool for Linux Sunshine kernels; it is never bundled in the app package.

To produce an installer on this machine (must match the target OS):

```bash
./scripts/package.sh          # Arch → .pkg.tar.zst, Debian → .deb, Fedora → .rpm
```

```powershell
.\scripts\package.ps1         # Windows 11 → NSIS setup.exe
```

Moonlight is pinned to commit `d2f6990b`, not tag `v6.1.0` (that tag does not build against FFmpeg 8). Details in the development guide.
