# User Guide

Eclipse is a peer-to-peer remote desktop node. One install is both a **host** (this PC can be controlled) and a **client** (this PC can control another). The window you see is the control plane; Sunshine and Moonlight do the actual capture, encode, decode, and input.

There are **no published GitHub Releases yet**. You can build a native package on each OS with [`scripts/package.sh`](../scripts/package.sh) / [`scripts/package.ps1`](../scripts/package.ps1), or run from source (same path as [Development.md](Development.md)). Video capture, encode, and smoke tests: [Video.md](Video.md).

## What works today

- A packaged install starts Sunshine at boot (`eclipse-host.service` on Linux, `SunshineService` on Windows). You do not need to open Eclipse for this PC to be a host.
- After you log in, an **Eclipse tray icon** appears. **Host streaming** is on by default; uncheck it to pause the host without uninstalling. The icon stays so you can turn it back on. A reboot turns the host on again.
- Closing the Eclipse window hides it to the tray. **Quit Eclipse** in the tray menu exits the app and leaves the packaged host service running (or stopped, if you toggled it off).
- Opening Eclipse shows whether the Sunshine and Moonlight binaries are present and whether the host process answers on `:47990`.
- If Tailscale is installed and logged in, **Peers** lists other machines on the tailnet and whether Sunshine answers on port `47990`.
- **Connect** on a peer with Eclipse host online pairs (if needed) and opens a Moonlight window on Desktop.
- Sunshine's own web UI is at [https://localhost:47990](https://localhost:47990) (self-signed certificate; the browser warning is expected). If Sunshine has no username yet, packages seed `eclipse` / `eclipse`.

## What is not built yet

- GitHub Actions published installers (local `package.sh` / `package.ps1` exist)
- Prompting for a custom Sunshine Web UI password (Connect expects the seeded `eclipse` / `eclipse` credentials)

Two Eclipse machines on the same tailnet can Connect. A packaged host stays online after reboot without anyone logging in. The tray is only present after a desktop login. Source builds use the tray to start/stop a Sunshine child (no system service). Linux input injection still needs the `input` group (packages add it; log out once).

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
| NVIDIA | NVENC | Proprietary NVIDIA driver. CUDA Toolkit is **not** required to *run* NVENC and is not included in Eclipse packages. |

Check Linux encode support:

```bash
vainfo
```

You want encoder entries such as `H264`, `HEVC`, or `AV1` (Mesa `radeonsi`, Intel media, or NVIDIA depending on the GPU). If `vainfo` is missing or shows no encoders, install the driver packages above and reboot if the kernel module was new.

On Windows, use the vendor's current Game Ready / Adrenalin / Arc driver.

### Capture and audio (Linux)

Sunshine captures via the session compositor when a desktop is running. A packaged `eclipse-host.service` also runs **before login**; capture stays on **auto**, so KMS is the fallback at the greeter. Details and smoke scripts: [Video.md](Video.md).

- **Wayland:** PipeWire / desktop portal, or Sunshine's native `wlgrab` backend (wlr-screencopy; used on Hyprland). Keep capture on **auto**. Do not ship `capture = kms`.
- **X11:** X11 capture APIs.
- **Windows:** DXGI Desktop Duplication (no KMS), including the login screen when `SunshineService` is installed.

A packaged Linux host can serve the login greeter over KMS. After you log into Wayland, KMS remains the fallback and may be weaker than a GUI-child `wlr` session. Headless machines with no GPU/DRM still cannot host a desktop stream.

### Keyboard and mouse injection (Linux hosts)

Sunshine injects input through `/dev/uinput`. Native Eclipse packages install udev rules, `setcap` the Sunshine sidecar, and add logged-in users to `input`. Log out once after install. For a source build, [`scripts/linux-host-perms.sh`](../scripts/linux-host-perms.sh) does the same into `/etc`.

On a Linux host you will use for remote control:

```bash
./scripts/linux-host-perms.sh
./scripts/linux-host-perms.sh --status
```

Log out of the desktop session (or reboot) so the new group applies. Without this, video may work while mouse and keyboard on the remote PC do not.

Confirm the kernel module is available:

```bash
sudo modprobe uinput
ls -l /dev/uinput
```

### Network identity (required to see peers)

The architecture uses **Tailscale** as the LAN. Install Tailscale from your OS and log in (`tailscale up`) on every PC that should appear as a peer. Eclipse does not wrap that login; it only runs `tailscale status --json`.

For a machine that just powered on to be reachable **before anyone logs in**, Tailscale must also be a system service (the default for the official Tailscale installers) and must have completed `tailscale up` once so credentials persist. Eclipse's host service starts independently of the GUI; without `tailscaled` up, peers cannot route.

You do not need Tailscale to verify that Sunshine starts locally. Without it, Peers shows an error instead of a list.

Host-online on a peer means TCP to `:47990` succeeded. If the peer’s Sunshine is bound only to localhost, it will show as on the tailnet but “No Sunshine.”

### Runtime libraries

When you run from source, [Development.md](Development.md) installs these. Linux packages declare them (WebKitGTK, GTK, Qt 6, FFmpeg, libva, PipeWire, SDL2, and so on). NVIDIA encode still needs the **vendor driver**, not the CUDA toolkit.

You should **not** install standalone Sunshine or Moonlight from your distro if you are using Eclipse. Eclipse vendors and launches its own copies; a second system Sunshine on port `47990` will conflict.

---

## 2. Install and run

### From a local package

Build on the same OS family you will install (Arch package on Arch, `.deb` on Ubuntu 24.04 / Debian, `.rpm` on Fedora, NSIS on Windows 11):

```bash
./scripts/package.sh
```

```powershell
.\scripts\package.ps1
```

Then install the file it prints (`pacman -U`, `apt install ./…deb`, `dnf install ./…rpm`, or the NSIS setup exe). On Linux, log out once so the `input` group applies. The host service is enabled at install time (`eclipse-host.service` / `SunshineService`). Install Tailscale and run `tailscale up` once so the node is on the tailnet after reboot.

Do **not** install official Sunshine alongside Eclipse: both use SCM name `SunshineService` on Windows and port `47990` everywhere.

### From source

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

1. After a **package** install, Sunshine is already running. After login, the Eclipse tray starts in the background (`eclipse --tray`).
2. Left-click the tray (or **Open Eclipse**) for the dashboard. The window close button hides to the tray.
3. Right-click the tray: **Host streaming** pauses or resumes the host service. **Quit Eclipse** exits the tray app; it does not uninstall the boot service.
4. From source, `npm run tauri -- dev` shows the window and tray. Host streaming toggles the Sunshine child.
5. Optional: open [https://localhost:47990](https://localhost:47990) to finish Sunshine's first-run username/password if prompted.

**Connect** on a peer row starts pairing if needed, then opens a Moonlight window on the host Desktop. Open the cog next to **HOSTS** for stream settings (left is lower quality, right is higher): codec (H.264, HEVC, or AV1), scale (Quarter / Half / Native of the host monitor), 30 or 60 fps, and a 10 / 20 / 50 Mb/s cap. Defaults are H.264, Native, 60 fps, and 20 Mb/s. The scale picker shows the resulting encode size for each known host display; the host desktop mode does not change. Choices are saved and used on the next Connect; if you are already streaming, Eclipse respawns the Moonlight window. The **!** next to **CLIENT** lists in-stream shortcuts. Disconnect from the dashboard, or use the last window / Ctrl+Alt+Esc. **CLIENT** **Status** is Idle, currently controlling a host, or currently streaming to a client (Connect is disabled until you **Force disconnect from streaming client**). Linux input injection still needs the `input` group.

While a Moonlight window is focused:

| Shortcut | Action |
| --- | --- |
| Ctrl+Alt+Esc | Close every window and disconnect |
| Ctrl+Alt+F | Fullscreen this window (letterboxed, no stretched pixels) |
| Ctrl+Alt+1–9 | Switch this window to host display N |
| Ctrl+Alt+0 | One client window per host display |
| Ctrl+Alt+Q | Close this window (last window disconnects) |

Closing the last stream window ends the host Desktop app. Extra displays cost extra encode and bandwidth; Eclipse caps Ctrl+Alt+0 at four windows.

On Hyprland / Omarchy, the tray appears only if the bar has a tray module (for example Waybar `tray`).

### After a packaged install

1. Install Tailscale as a system service and complete `tailscale up` once.
2. On Linux, log out once if you just installed (udev / `input` group). Source builds still use `./scripts/linux-host-perms.sh`.
3. Confirm the host is up without opening Eclipse: `systemctl is-active eclipse-host.service` (Linux) or Services → Eclipse Host (Windows).
4. On a **client** PC, open **Eclipse**, pick a peer, and click **Connect**.

---

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| No tray icon after login | Packaged autostart uses `eclipse --tray`. On Windows, check HKLM Run `Eclipse`. On Linux, `/etc/xdg/autostart/eclipse-tray.desktop`. Hyprland needs a bar tray module. |
| Windows setup window closes immediately | Do not double-click the script. From PowerShell in the repo: `.\scripts\setup-windows.ps1`. The UAC helper used to fail on Qt 6.8 extra-module names; the error is also in `%TEMP%\eclipse-windows-setup.log`. |
| `cargo` / Tauri error about missing `binaries/sunshine` | The sidecar files are gitignored. You must compile them on this machine (or copy the correctly named triple-suffixed binaries into `src-tauri/binaries/`). |
| No hardware encoders | `vainfo` on Linux; GPU drivers on Windows. AMD on this project used Mesa VA-API (`h264_vaapi` / `hevc_vaapi` / `av1_vaapi`), not CUDA. |
| Browser warning on port 47990 | Expected. Sunshine uses a self-signed certificate. |
| Host process pill stays off | Packaged: Host streaming is off in the tray, or the host service is not running (`systemctl status eclipse-host.service` / Windows `SunshineService`). From source: sidecars were not built, or Sunshine failed to spawn. |
| Port 47990 already in use | Another Sunshine is running, or Host streaming is still on. Toggle it off in the tray, or stop `eclipse-host.service` / `SunshineService`. |
| Peers says Tailscale is not installed / not running | Install Tailscale and run `tailscale up`. Eclipse does not log you in. |
| Peer is on the tailnet but “No Sunshine” | The other PC’s host service is not running, or Sunshine is not listening on `:47990`. On a packaged host you do not need Eclipse open. On Windows, confirm `https://localhost:47990` loads on the host. Windows Firewall may still block inbound GameStream ports on the Tailscale adapter. |
| Connect says PIN was not accepted before timeout | On Windows, Moonlight needs Qt/SDL/FFmpeg DLLs next to it — rebuild sidecars and restart Eclipse. On either OS, the host PC must have Sunshine up and GameStream ports reachable over Tailscale. |
| Moonlight: “No video received from host” / UDP 47998, 48000 | See [Video.md](Video.md). If audio arrived, UDP is open. Check the host Sunshine log for extra Sunshine on RTSP `48010`, `Couldn't import RGB Image: 00003009` (rebuild sidecars), or capture init failure. |
| Moonlight error 503 / “Failed to initialise video capture/encoding” | Host capture never started. If `capture = kms`, run `./scripts/linux-host-perms.sh` or `--reset-capture`. See [Video.md](Video.md). |
| Linux: `Couldn't import RGB Image: 00003009` | EGL DMA-BUF import failed on `wlr`. Rebuild sidecars so `patches/sunshine-egl-bad-match.patch` is applied. Do not force KMS unless `wlr` still fails. |
| Two copies of Sunshine | Uninstall distro Sunshine / close the other Eclipse window. GameStream RTSP is port `48010`; the Web UI is `47990`. Both must be the same process. |
| Connect says the host uses a custom Web UI password | That PC set a Sunshine password other than `eclipse` / `eclipse`. Eclipse does not prompt for it this version. |
| Connect opens Moonlight but input does not work (Linux) | Run `./scripts/linux-host-perms.sh`, then log out so the `input` group applies. Confirm `/dev/uinput` is writable. |
| Video works, input does not (Linux) | Same as above: `input` group and `/dev/uinput`. |

