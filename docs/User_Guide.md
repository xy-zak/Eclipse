# User Guide

Eclipse is a peer-to-peer remote desktop node. One install is both a **host** (this PC can be controlled) and a **client** (this PC can control another). The window you see is the control plane; Sunshine and Moonlight do the actual capture, encode, decode, and input.

There are **no published installers yet**. Until GitHub Actions ships `.deb` / `.rpm` / `.msi` packages, running Eclipse means building it from source. That is the same path as [Development.md](Development.md), with the runtime notes below.

## What works today

- Opening Eclipse starts a local Sunshine host sidecar.
- The dashboard shows whether the Sunshine and Moonlight binaries are present and whether the host process is running.
- If Tailscale is installed and logged in, **Peers** lists other machines on the tailnet and whether Sunshine answers on port `47990`.
- **Connect** on a peer with Eclipse host online pairs (if needed) and opens a Moonlight window on Desktop.
- Sunshine's own web UI is at [https://localhost:47990](https://localhost:47990) (self-signed certificate; the browser warning is expected). If Sunshine has no username yet, Eclipse seeds `eclipse` / `eclipse`.

## What is not built yet

- Packaged installers and post-install permission scripts
- Prompting for a custom Sunshine Web UI password (Connect expects the seeded `eclipse` / `eclipse` credentials)

Two Eclipse machines on the same tailnet can Connect. The host must be running Eclipse (Sunshine up). Linux input injection still needs the `input` group.

---

## 1. Prepare your system

These are the conditions the application needs on a machine that will host or view a desktop. Installers will eventually pull in libraries; they will not replace a working GPU encoder or (on Linux) input permissions.

### Operating system

- **Linux:** 64-bit Arch (including Omarchy), Debian/Ubuntu-family, or Fedora-family. Wayland and X11 are both in scope; this project was first verified on Omarchy (Hyprland / Wayland).
- **Windows:** Windows 11, 64-bit.

macOS is not a target yet.

### Hardware encoding and decoding

A working GPU encoder on the **host** is required for 60+ FPS. A working decoder on the **client** is required to display the stream.

| GPU | Host encode | Typical Linux packages (Arch names) |
| --- | --- | --- |
| AMD | VA-API / AMF | `mesa`, `vulkan-radeon`, `libva-utils` |
| Intel | VA-API / Quick Sync | `intel-media-driver`, `vulkan-intel`, `libva-utils` |
| NVIDIA | NVENC | Proprietary NVIDIA driver. CUDA Toolkit is **not** required to *run* NVENC; it is only needed if you compile Sunshine with CUDA yourself. |

Check Linux encode support:

```bash
vainfo
```

You want encoder entries such as `H264`, `HEVC`, or `AV1` (Mesa `radeonsi`, Intel media, or NVIDIA depending on the GPU). If `vainfo` is missing or shows no encoders, install the driver packages above and reboot if the kernel module was new.

On Windows, use the vendor's current Game Ready / Adrenalin / Arc driver.

### Capture and audio (Linux)

Sunshine captures via the session compositor:

- **Wayland:** PipeWire / desktop portal, or Sunshine's native `wlgrab` backend (wlr-screencopy; used on Hyprland during bootstrap).
- **X11:** X11 capture APIs.

PipeWire and a working desktop session are required. Headless machines without a graphical session cannot host a desktop stream.

### Keyboard and mouse injection (Linux hosts)

Sunshine injects input through `/dev/uinput`. Official Sunshine packages create the `input` group and install udev rules. Eclipse's local sidecar build **does not** install those rules into `/usr`.

On a Linux host you will use for remote control:

```bash
sudo groupadd -f input
sudo usermod -aG input "$USER"
# After the next login:
id -nG | grep -qw input && echo "input group: ok"
```

Log out of the desktop session (or reboot) so the new group applies. Without this, video may work while mouse and keyboard on the remote PC do not.

Confirm the kernel module is available:

```bash
sudo modprobe uinput
ls -l /dev/uinput
```

### Network identity (required to see peers)

The architecture uses **Tailscale** as the LAN. Install Tailscale from your OS and log in (`tailscale up`) on every PC that should appear as a peer. Eclipse does not wrap that login; it only runs `tailscale status --json`.

You do not need Tailscale to verify that Sunshine starts locally. Without it, Peers shows an error instead of a list.

Host-online on a peer means TCP to `:47990` succeeded. If the peer’s Sunshine is bound only to localhost, it will show as on the tailnet but “No Sunshine.”

### Runtime libraries

When you run from source, [Development.md](Development.md) installs these. When installers exist, Linux packages will declare them (WebKitGTK, GTK, Qt 6, FFmpeg, libva, PipeWire, SDL2, and so on).

You should **not** install standalone Sunshine or Moonlight from your distro if you are using Eclipse. Eclipse vendors and launches its own copies; a second system Sunshine on port `47990` will conflict.

---

## 2. Install and run

### Until installers ship: run from source

On each PC:

1. Clone the repository with submodules, then run the one-command setup (toolchain + sidecars + `npm install`). Use a real terminal on Linux (sudo / fingerprint). Windows will prompt for UAC once.

```bash
git clone --recurse-submodules <repo-url>
cd Eclipse
./scripts/setup-linux.sh
npm run tauri -- dev
```

Windows 11 (PowerShell):

```powershell
git clone --recurse-submodules <repo-url>
cd Eclipse
.\scripts\setup-windows.ps1
npm run tauri -- dev
```

First run can take a long time: Visual Studio Build Tools, Qt 6, and (on NVIDIA machines) the CUDA toolkit are multi-GB downloads. Set `ECLIPSE_CUDA=off` to skip CUDA. Bootstrap does not install NVIDIA kernel / Game Ready drivers — those stay an OS concern. Details: [Development.md](Development.md).

### First launch

1. An Eclipse window opens. Status pills should show Sunshine sidecar, Moonlight sidecar, and Host process.
2. Optional: open [https://localhost:47990](https://localhost:47990) to finish Sunshine's first-run username/password if prompted.
3. Close the Eclipse window to stop Sunshine. Leaving the window open keeps the host process running.

**Connect** on a peer row starts pairing if needed, then opens Moonlight on Desktop. The host PC must be running Eclipse. Linux input injection still needs the `input` group.

### After packaged installers exist

This section is the intended user path. It is not available yet.

1. Install Tailscale and complete `tailscale up`.
2. On Linux, ensure your user is in the `input` group (a post-install script is planned to do this).
3. Install the Eclipse package for your OS and open **Eclipse**.
4. Pick a peer and click **Connect**.

---

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Host process pill stays off | Sidecars were not built, or Sunshine failed to spawn. Build with `./scripts/build-sidecars.sh` and look at the terminal that launched `npm run tauri -- dev`. |
| `cargo` / Tauri error about missing `binaries/sunshine` | The sidecar files are gitignored. You must compile them on this machine (or copy the correctly named triple-suffixed binaries into `src-tauri/binaries/`). |
| No hardware encoders | `vainfo` on Linux; GPU drivers on Windows. AMD on this project used Mesa VA-API (`h264_vaapi` / `hevc_vaapi` / `av1_vaapi`), not CUDA. |
| Browser warning on port 47990 | Expected. Sunshine uses a self-signed certificate. |
| Port 47990 already in use | Another Sunshine (or leftover Eclipse) is running. Close Eclipse or stop that process. |
| Peers says Tailscale is not installed / not running | Install Tailscale and run `tailscale up`. Eclipse does not log you in. |
| Peer is on the tailnet but “No Sunshine” | The other PC is not running Eclipse, or Sunshine is bound only to localhost. |
| Connect says the host uses a custom Web UI password | That PC set a Sunshine password other than `eclipse` / `eclipse`. Eclipse does not prompt for it this version. |
| Connect opens Moonlight but input does not work (Linux) | User not in `input` group, or `/dev/uinput` not writable. Log out after `usermod`. |
| Video works, input does not (Linux) | User not in `input` group, or `/dev/uinput` not writable. Log out after `usermod`. |
| Two copies of Sunshine | Uninstall distro Sunshine / close the other Eclipse window. |
