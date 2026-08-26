# Development

How to ready a workstation and run Eclipse from this repository. Runtime conditions (GPU encode, `input` group, Tailscale) are in [User_Guide.md](User_Guide.md). Product intent is in [Architecture_Specification.md](Architecture_Specification.md).

## What this repo is today

| Piece | State |
| --- | --- |
| Tauri 2 + React 19 + Vite + Tailwind v4 | Scaffolded (`com.eclipse.app`) |
| Sunshine sidecar | Vendored at **v2026.824.30334**, compiled locally. Packages run it as a boot-time service; `tauri dev` spawns a child |
| Moonlight-Qt sidecar | Vendored at commit **`d2f6990b`**, compiled locally, spawned on Connect |
| Dashboard | CLIENT status (Tailscale / Sunshine / Moonlight), CLIENT menus (Moonlight / Sunshine / Tailscale / shortcuts), Tailscale **Peers** list, per-row **Connect**, inbound stream on **CLIENT**, Host streaming toggle (tray and CLIENT sun menu) |
| Tailscale discovery | `tailscale status --json` + TCP `:47990` probe (`discovery.rs`) |
| Pairing / stream launch | `pairing.rs` POST `/api/pin`; `session.rs` `list` / `pair --pin` / `stream … Desktop` |
| GitHub Actions installers | Not implemented (local `scripts/package.sh` / `package.ps1` first) |

From source, **Quit Eclipse** in the tray stops the Sunshine child unless **Keep Sunshine Host running in background** is on. Closing the window only hides it. Packaged installs default to leaving `eclipse-host.service` / `SunshineService` running after quit; reboot starts the host again. The UI still opens if Sunshine is missing or fails to start. `eclipse --tray` starts hidden.

---

## 1. Prepare your system

You need a **compile toolchain** plus the same GPU/session conditions as a user. Sidecar binaries are gitignored; every developer machine compiles them (or copies the correctly named files for its Rust host triple).

### Tooling (all platforms)

| Tool | Role | Notes |
| --- | --- | --- |
| Git | Clone + submodules | Recurse into `vendor/` |
| Node.js LTS + npm | Vite frontend | Already present via mise on some Omarchy setups |
| Rust stable (`rustup`) | Tauri | Verified with rustc 1.98; `rustup default stable` |
| CMake + Ninja | Sunshine | Ninja generator is required by the Linux script and the Windows script |
| Qt 6 (`qmake6` / `qmake`) | Moonlight-Qt | Widgets, Quick, SVG, Wayland plugin on Linux |
| C/C++ compiler | Engines | `base-devel` / MSVC |

CUDA Toolkit (`nvcc`) is installed **as needed** by bootstrap when an NVIDIA GPU is present. On **Linux**, the sidecar build turns Sunshine CUDA **on** when `nvcc` is on `PATH`, unless you set `ECLIPSE_CUDA=off`. Force toolkit install / CUDA compile with `ECLIPSE_CUDA=on`. On **Windows**, Sunshine encodes with NVENC; `-DSUNSHINE_ENABLE_CUDA` is a Linux-only CMake option and is not passed. Bootstrap does **not** install NVIDIA kernel or Game Ready drivers.

Do **not** check out Moonlight tag `v6.1.0` on current Arch: it fails against FFmpeg 8 (`AVCodec::pix_fmts` removed, Vulkan hwcontext field renames). The tracked commit is post-v6.1.0. Re-pin when Moonlight cuts a release that builds on FFmpeg 8.

### Linux packages

From the repo root, in a **real terminal** (sudo, fingerprint, or polkit need a TTY):

```bash
./scripts/setup-linux.sh        # bootstrap + sidecars + npm install
# or only the toolchain:
./scripts/bootstrap.sh
```

The bootstrap script picks packages from `/etc/os-release`:

- **Arch / Omarchy / Manjaro:** `omarchy pkg add` when `omarchy` exists, otherwise `pacman`. Installs Tauri deps (WebKitGTK 4.1, GTK3, appindicator, rsvg), Sunshine/Moonlight deps (CMake, Ninja, Qt 6, FFmpeg, VA-API, PipeWire, SDL2, …), and both Intel and AMD VA-API/Vulkan packages.
- **Debian / Ubuntu:** `apt` equivalents, then rustup if `rustc` is missing, and Node 22 into `~/.local` if the distro Node is missing or older than 20.
- **Fedora:** `dnf` equivalents, including `rustup`.

If an NVIDIA GPU is present (and `ECLIPSE_CUDA` is not `off`), bootstrap also installs the CUDA toolkit (`cuda` / `nvidia-cuda-toolkit` / `cuda-nvcc` depending on distro). It does not install the NVIDIA kernel driver.

`scripts/bootstrap.sh` is the source of truth for package names. After it finishes it prints `rustc`, `cargo`, `node`, `npm`, `cmake`, `ninja`, `qmake6`, CUDA/`nvcc`, and a `vainfo` snippet.

#### Omarchy

`omarchy pkg add` will fail in a non-interactive or sandboxed session (no fingerprint sudo). Run bootstrap in your desktop terminal. Node may already come from mise; that is fine.

### Windows 11 packages

`scripts/bootstrap.ps1` **installs** missing tools (self-elevates once via UAC). Cold install is large: VS 2022 Build Tools, Qt 6 MSVC via aqtinstall, CUDA toolkit on NVIDIA machines, and **MSYS2 UCRT64** for Sunshine.

It uses `winget` for Git, Node LTS, CMake, Ninja, rustup, VS Build Tools (C++ workload), and MSYS2; Python + aqtinstall for Qt 6.8.3 MSVC (Moonlight); `Nvidia.CUDA` when an NVIDIA GPU is present. Sunshine on Windows is **not** an MSVC build — LizardByte compiles it with MinGW UCRT64 (`mingw-w64-ucrt-x86_64-openssl` and related pacman packages). Skip CUDA with `ECLIPSE_CUDA=off`. It does **not** install the Game Ready driver.

`scripts/windows-dev-env.ps1` refreshes PATH, locates MSYS2, and runs Moonlight `nmake` inside `vcvars64.bat` (`Invoke-EclipseNmake`) so PowerShell never imports the full VsDevCmd environment. `build-sidecars.ps1` builds Sunshine inside UCRT64 and Moonlight with MSVC `nmake`.

```powershell
.\scripts\setup-windows.ps1     # bootstrap + sidecars + npm install
# or only the toolchain:
.\scripts\bootstrap.ps1
```

### Linux host permissions (same as users)

If this machine will host a stream or inject input:

```bash
./scripts/linux-host-perms.sh
./scripts/linux-host-perms.sh --status
```

That setcaps every Sunshine binary it finds (Tauri copies strip capabilities), installs udev rules into `/etc`, and adds the user to `input`. Re-run after sidecar rebuilds and after the first `tauri dev` copy. Details: [Video.md](Video.md).

The sidecar CMake install still writes udev/systemd units into `src-tauri/resources/sunshine-prefix/` as well; that prefix is not loaded by the system. Packaged Linux installs Eclipse's own `eclipse-host.service` instead. `linux-host-perms.sh` does not enable that unit (source trees keep the GUI child).

---

## 2. Install and run the application (from source)

### Clone

```bash
git clone --recurse-submodules <repo-url>
cd Eclipse
./scripts/setup-linux.sh
```

If you already cloned without submodules:

```bash
git submodule update --init --recursive
```

Use this only to **fetch** missing nested repos. Do not follow it with a checkout of Moonlight `v6.1.0`.

Expected pins are in `vendor/PINS` (enforced by `scripts/check-pins.sh` and CI):

- `vendor/sunshine` → tag **v2026.824.30334**
- `vendor/moonlight-qt` → **d2f6990b** (`Send LI_CTYPE_STEAM for known Steam Controllers`)

```bash
./scripts/check-pins.sh
```

### Build the sidecars

`setup-linux.sh` / `setup-windows.ps1` already do this. To rebuild only the engines:

Linux:

```bash
./scripts/build-sidecars.sh
```

Windows (any PowerShell; VsDevCmd is imported automatically):

```powershell
.\scripts\build-sidecars.ps1
```

This compiles Sunshine (CMake + Ninja, CUDA auto) and Moonlight (`qmake6` + `make` / `nmake`), then copies:

```
src-tauri/binaries/sunshine-<rust-host-triple>      # .exe on Windows
src-tauri/binaries/moonlight-<rust-host-triple>
src-tauri/resources/sunshine/                       # web UI, shaders, assets
src-tauri/resources/moonlight-runtime/              # Windows Qt/SDL/FFmpeg (gitignored; from windeployqt)
```

Example Linux triple: `x86_64-unknown-linux-gnu`. Tauri `externalBin` is `binaries/sunshine` and `binaries/moonlight`; the build system appends the triple. `cargo` / `tauri dev` fail with a missing-file error until these exist.

Sunshine is installed into a local prefix `src-tauri/resources/sunshine-prefix/` (gitignored) so asset paths do not point at `/usr`. CMake is passed `UDEV_RULES_INSTALL_DIR` (and systemd dirs) inside that prefix. Upstream `FindUdev.cmake` otherwise overwrites those dirs from pkg-config (`/usr/lib/udev`); [`patches/sunshine-udev-prefix.patch`](../patches/sunshine-udev-prefix.patch) plus a rewrite of `cmake_install.cmake` keep install inside `PREFIX`. Before configure, both sidecar scripts apply `patches/sunshine-*.patch` and `patches/moonlight-*.patch` (see [`patches/README.md`](../patches/README.md)).

Rebuild sidecars after pulling engine submodule updates, after changing `ECLIPSE_CUDA`, or after changing a patch. TTF fonts are copied from `assets/fonts/` into Moonlight `app/res/eclipse/` at build time; they are not in the patch files.

#### Sidecar patches

Series, apply order, and bump SOP: [`patches/README.md`](../patches/README.md). Refresh a dirty vendor tree with `./scripts/refresh-sidecar-patches.sh` (Windows: `.\scripts\refresh-sidecar-patches.ps1`). CI runs `./scripts/check-sidecar-patches.sh` so a patch that no longer applies on the pin fails before a full sidecar compile.

Windows notes that are easy to trip over:

- SCM service name **must** stay `SunshineService` (hardcoded in `vendor/sunshine/tools/sunshinesvc.cpp`). Do not install official Sunshine next to Eclipse.
- `build-sidecars.ps1` rewrites the Sunshine PE subsystem to WINDOWS so the host does not flash a console.
- `src-tauri/resources/moonlight-runtime/` is Windows-only (`tauri.windows.conf.json` + `windeployqt`). Linux packages do not bundle it.

Linux packages share [`packaging/linux/host-setup.sh`](../packaging/linux/host-setup.sh) (`postinst.sh` and Arch `eclipse.install` both call it).

### Run the control plane

```bash
npm install
npm run tauri -- dev
```

Vite serves the UI at `http://localhost:1420`. Tauri opens the Eclipse window and tries to spawn Sunshine. On a working VA-API host you should see `[wlgrab]` and encoder lines such as `h264_vaapi` in the Sunshine log, and the web UI at [https://localhost:47990](https://localhost:47990).

Typecheck / compile without launching:

```bash
npx tsc --noEmit
# sidecars must already exist:
cd src-tauri && cargo check
```

Release packaging is local, not a CI matrix. From the repo root on the **same OS** you want to install:

```bash
./scripts/package.sh                 # Arch, Debian/Ubuntu 24.04, or Fedora
# --skip-sidecars if binaries/sunshine-<triple> is already built
```

```powershell
.\scripts\package.ps1
```

| OS | Output |
| --- | --- |
| Arch (this machine) | `packaging/arch/eclipse-*.pkg.tar.zst` — `sudo pacman -U` that file |
| Debian / Ubuntu 24.04 | `src-tauri/target/release/bundle/deb/*.deb` |
| Fedora | `src-tauri/target/release/bundle/rpm/*.rpm` |
| Windows 11 | `src-tauri/target/release/bundle/nsis/*-setup.exe` |

Do not build a Fedora RPM on Arch (or a Debian package on Fedora): sidecar linkage follows the build distro. CUDA toolkit is used only if `nvcc` is present while compiling Linux Sunshine; it is **not** a runtime dependency. NVIDIA users need the vendor driver.

Linux packages install udev rules, `setcap` the Sunshine sidecar, and add logged-in users to `input`. Log out once after install. Source trees still use [`scripts/linux-host-perms.sh`](../scripts/linux-host-perms.sh).

### Layout

```
Eclipse/
  docs/                         # this guide, user guide, architecture spec
  packaging/                    # deb/rpm maintainer scripts, Arch PKGBUILD, NSIS hooks
  patches/                      # sidecar series; see patches/README.md
  scripts/                      # bootstrap, setup, sidecar build, package.sh / package.ps1
  src/                          # React + Vite + Tailwind
  src-tauri/
    binaries/                   # gitignored sidecars
    resources/sunshine/         # copied Sunshine assets
    resources/sunshine-prefix/  # gitignored cmake prefix
    resources/moonlight-runtime/ # Windows Moonlight Qt runtime (gitignored)
    src/
      lib.rs                    # Tauri setup; sidecar_status, list_peers, connect
      sidecar.rs                # Engine adapter (resolve / spawn / health / --creds)
      hostconf.rs               # sunshine.conf merge, CGNAT checks, Web UI creds, TLS pins
      discovery.rs              # Tailscale peers + Sunshine TCP probe
      pairing.rs                # POST /api/pin; GET /api/displays; GET /api/sessions; RTSP preflight
      display.rs                # local monitors + integer-scale table
      session.rs                # Moonlight list / pair / stream windows
    tauri.conf.json
  vendor/PINS                    # required submodule SHAs
  vendor/sunshine/              # submodule
  vendor/moonlight-qt/          # submodule
```

Rust commands: `sidecar_status` → `{ sunshinePresent, moonlightPresent, sunshineRunning, hostTriple }`; `list_peers` → `{ peers, tailscaleOk, message }`; `inbound_status` → `{ ip }` when this PC is being streamed to; `force_disconnect_inbound` → POST `/api/apps/close` on localhost; `connect` → `moonlight list` / `pair --pin` + POST `/api/pin` / `stream --resolution WxH --video-codec H.264 … Desktop`. Sunshine is started only via `Engine` + `std::process::Command`. Before spawn, Eclipse copies `resources/sunshine` to `{exe_dir}/share/sunshine` and sets cwd to the binary’s parent — Windows Sunshine is compiled with that relative path and does not honor `SUNSHINE_ASSETS_DIR` at runtime. On Windows, Connect also copies `resources/moonlight-runtime` beside `moonlight.exe` so Qt/SDL/FFmpeg DLLs load. If Sunshine has no Web UI credentials (or still has the old `eclipse`/`eclipse` pair), Eclipse runs `sunshine --creds eclipse <random>` and writes `web-ui-password` next to `sunshine_state.json`. Resolution order: file named `sunshine` beside the debug exe, else `src-tauri/binaries/sunshine-<HOST_TRIPLE>`.

Adapter unit tests: `cd src-tauri && cargo test --lib`

Sunshine HTTPS contract (stop Eclipse first if port 47990 is taken):

```bash
./scripts/smoke-sunshine.sh
# video (needs a desktop session):
./scripts/smoke-video.sh loopback
# or:
ECLIPSE_SMOKE=1 cargo test --manifest-path src-tauri/Cargo.toml --lib sunshine_smoke
```

Windows video smoke: `.\scripts\smoke-video.ps1 loopback`. Capture/encode notes: [Video.md](Video.md).

---

## Troubleshooting (developer)

| Symptom | Cause / fix |
| --- | --- |
| `omarchy pkg add` / `sudo` fails in the agent | Run `./scripts/bootstrap.sh` in a desktop terminal. |
| Tauri build: sidecar file not found | Run `./scripts/build-sidecars.sh`. Filenames must include the host triple. |
| Sunshine spawn ENOENT | Dev copies are named `sunshine` next to the debug exe; source copies are `sunshine-<triple>` under `binaries/`. `sidecar.rs` `Engine::resolve` handles both. |
| Moonlight fails on `pix_fmts` / Vulkan hwcontext | You are on Moonlight `v6.1.0` + FFmpeg 8. Return to the SHA in `vendor/PINS`. |
| `git submodule update --recursive` unpinned Moonlight | Sidecar scripts only init submodules when sources are missing. Confirm `./scripts/check-pins.sh`. |
| Sunshine `cmake --install` wanted `/usr/lib/udev` | `FindUdev.cmake` ignores `-DUDEV_*` unless patched. Re-run `./scripts/build-sidecars.sh` (it applies `patches/sunshine-udev-prefix.patch` and rewrites `cmake_install.cmake`). Do not `sudo make install`. |
| Windows `setup-windows.ps1` window closes / UAC flash | Qt 6.8 extra-module names (`qtdeclarative`, `qtsvg`) are in the base kit; passing them as `aqt -m` extras fails immediately. Re-run from a PowerShell prompt (not by double-clicking). Full text: `%TEMP%\eclipse-windows-setup.log`. |
| Sunshine CMake `Could NOT find OpenSSL` on Windows | Sunshine must be configured with MSYS2 UCRT64, not MSVC. Re-run `.\scripts\setup-windows.ps1` so bootstrap installs MSYS2 + `mingw-w64-ucrt-x86_64-openssl`. Delete `build\sunshine` if it was configured with MSVC. |
| Sunshine configure stops at `Got from CI ... version` | CMake prints that on stderr. PowerShell `Stop` was treating it as a fatal error. Re-run `.\scripts\setup-windows.ps1`. |
| `nmake` not on PATH / `The input line is too long` | Do not dump VsDevCmd into PowerShell. `Invoke-EclipseNmake` runs `vcvars64.bat && nmake` in a child cmd after clearing leftover INCLUDE/LIB/PATH. Install VS Build Tools (MSVC) if `vcvars64.bat` is missing. |
| Port 47990 in use | Another Eclipse/Sunshine still running, or the host service is up. Quit Eclipse (source child) or toggle Host streaming off / stop `eclipse-host.service`. |
| Windows `https://localhost:47990` empty / `netstat` has no 47990 | Sunshine exited on first start because `share/sunshine/apps.json` was missing beside the exe. Restart Eclipse after a sidecar rebuild so `sidecar.rs` stages that layout. |
| Windows Connect: PIN timeout / Moonlight never appears | `moonlight.exe` needs Qt/SDL/FFmpeg DLLs next to it. Re-run `.\scripts\build-sidecars.ps1` so `windeployqt` fills `src-tauri/resources/moonlight-runtime/`. |
| `list_peers` / Peers error | `tailscale` must be on `PATH` and logged in (`tailscale status --json`). |
| Connect 401 on `/api/pin` | Host sidecar lacks `patches/sunshine-cgnat-pair.patch`. Rebuild sidecars on that PC. |
| Connect prompts to trust a host certificate | Expected after a full reinstall on that host (new Sunshine self-signed cert). **Trust this Host** replaces the pin. App updates do not rotate the cert. |
| Connect hangs then errors on PIN | Host Sunshine must be running (package service or source child); Moonlight pair must reach GameStream ports over Tailscale (`100.64.0.0/10`). |
| Windows Connect PIN timeout while Linux→Windows works | Windows Moonlight must add Tailscale IPv4 hosts via `QHostAddress` (not `QUrl`). Rebuild Moonlight with `.\scripts\build-sidecars.ps1` so `nmake` runs inside `vcvars64`. Also confirm `moonlight-runtime\platforms\qwindows.dll` is packaged. |
| Moonlight “No video received” / UDP 47998 | See [Video.md](Video.md). Check host Sunshine for `Couldn't bind RTSP server to port [48010]`, `Couldn't import RGB Image: 00003009`, or capture init failure. |
| Moonlight 503 / KMS `CAP_SYS_ADMIN` | `capture = kms` without setcap. `./scripts/linux-host-perms.sh` or `--reset-capture`. |

---

## Next product work (not done)

- GitHub Actions installer matrix and GitHub Releases (after local packages are confirmed)
