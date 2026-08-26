# Video streaming

How Eclipse captures, encodes, and tests GameStream video on Linux and Windows. Runtime setup lives in [User_Guide.md](User_Guide.md); build steps in [Development.md](Development.md).

Capture stays on **auto**. Do not ship `capture = kms` as a default. KMS is a Linux fallback that needs extra privileges. Packaged `eclipse-host.service` relies on that fallback at the login greeter (no wlr/portal session yet).

## Per OS

| | Linux | Windows |
| --- | --- | --- |
| Capture (auto) | `wlr` / `wlgrab` on Hyprland and other wlroots compositors; XDG Portal + PipeWire on GNOME/KDE; **KMS** only if those fail and the Sunshine binary has `CAP_SYS_ADMIN` | DXGI Desktop Duplication (Sunshine default; there is no KMS) |
| Encode | VA-API (AMD/Intel Mesa), NVENC, or software | NVENC, AMF, Quick Sync, Media Foundation |
| Client decode | VA-API / Vulkan in Moonlight | DXVA2 / D3D11VA |
| Host privileges | Packages: `eclipse-host.service` (root, `CAP_SYS_ADMIN` via setcap), udev, `input` group. Source: [`scripts/linux-host-perms.sh`](../scripts/linux-host-perms.sh) | Packages: NSIS firewall + `SunshineService`. Source: [`scripts/windows-firewall.ps1`](../scripts/windows-firewall.ps1) |
| Stream contract | Moonlight `stream --resolution WxH --fps 60 --video-codec H.264 --no-hdr --packet-size 1024 --absolute-mouse --display-mode windowed --host-display N <ip> Desktop`. `WxH` is Native, Half, or Quarter of the host display’s physical current mode ([`session.rs`](../src-tauri/src/session.rs)). Host inventory: Sunshine `GET /api/displays`. | Same |

Portal capture needs a working `RemoteDesktop` / ScreenCast portal. Hyprland’s portal often lacks `org.freedesktop.portal.RemoteDesktop`; that is expected. Prefer `wlr` there.

## Linux capabilities and Tauri copies

`setcap cap_sys_admin,cap_sys_nice+p` is required **only for KMS** (and helps encoder niceness). `wlr` does not need it once the EGL import fallback is in the sidecar.

Tauri copies `externalBin` to `src-tauri/target/debug/sunshine` and **strips file capabilities**. `setcap` on `src-tauri/binaries/sunshine-<triple>` is not enough for `npm run tauri -- dev`. `linux-host-perms.sh` caps every Sunshine binary it finds. Re-run it after:

- `./scripts/build-sidecars.sh`
- the first `tauri dev` copy of `sunshine`

```bash
./scripts/linux-host-perms.sh           # sudo: caps, udev, input group
./scripts/linux-host-perms.sh --status
./scripts/linux-host-perms.sh --reset-capture   # drop capture = kms from sunshine.conf
```

The `input` group needs a new desktop session. Packaged installs run `postinst` / `eclipse.install`; source trees use this script.

`--reset-capture` drops `capture = kms`. For a source-tree child, quit Eclipse after reset so the running process does not keep KMS in memory. For `eclipse-host.service`, restart the unit after editing `/etc/eclipse/sunshine/sunshine.conf`.

## EGL `00003009` (Linux `wlr`)

`Couldn't import RGB Image: 00003009` is EGL `EGL_BAD_MATCH`. Sunshine imported a DMA-BUF with a modifier Mesa rejected. [`patches/sunshine-egl-bad-match.patch`](../patches/sunshine-egl-bad-match.patch) retries the import without the modifier. Host display inventory, `GET /api/displays`, and per-display capture are [`patches/sunshine-displays.patch`](../patches/sunshine-displays.patch). Inbound session list and force-disconnect (`GET /api/sessions`, `POST /api/apps/close` also terminates RTSP) are [`patches/sunshine-sessions.patch`](../patches/sunshine-sessions.patch). Moonlight stream contract, Tailscale IP connect, and the connecting splash are [`patches/moonlight-01-behavior.patch`](../patches/moonlight-01-behavior.patch) and [`patches/moonlight-02-ui.patch`](../patches/moonlight-02-ui.patch). `build-sidecars.sh` / `.ps1` apply `patches/sunshine-*.patch` and `patches/moonlight-*.patch`; see [`patches/README.md`](../patches/README.md). Rebuild sidecars to pick them up. The submodule SHA in `vendor/PINS` does not change.

## Smoke tests

HTTPS-only: [`scripts/smoke-sunshine.sh`](../scripts/smoke-sunshine.sh).

Video (graphical session required for `wlr`/portal; packaged Linux hosts can also be probed at the greeter via KMS):

```bash
./scripts/smoke-video.sh              # host: spawn or attach, check encoder + TCP 47990/48010
./scripts/smoke-video.sh client <ip>  # pair if needed, stream 10s windowed H.264 (default 1920×1080; override with ECLIPSE_SMOKE_RESOLUTION=WxH)
./scripts/smoke-video.sh loopback     # host then client 127.0.0.1
```

Windows:

```powershell
.\scripts\smoke-video.ps1
.\scripts\smoke-video.ps1 client 100.x.x.x
.\scripts\smoke-video.ps1 loopback
```

Transcript: `/tmp/eclipse-video-smoke.log` or `%TEMP%\eclipse-video-smoke.log`. Override duration with `ECLIPSE_VIDEO_SMOKE_SECS`. Pairing uses the same `eclipse` / `eclipse` Web UI credentials as Connect.

## Log signatures

| Line | Meaning |
| --- | --- |
| `Found H.264 encoder: h264_vaapi` / `h264_nvenc` | Hardware encode is available |
| `[wlgrab] Selected monitor` | Wayland wlr capture |
| `Screencasting with Wayland's protocol` | wlgrab/portal path |
| `CLIENT CONNECTED` | Moonlight completed the handshake |
| `Couldn't import RGB Image: 00003009` | EGL import failed; rebuild sidecars with the patch, do not assume a firewall hole |
| `Failed to gain CAP_SYS_ADMIN` with `capture = kms` | KMS without `setcap`; run `linux-host-perms.sh` or `--reset-capture` |
| `Unable to initialize capture method` | No working capture; Moonlight then reports **error 503** |
| Moonlight `No video received from host` | No video UDP. If audio arrived, UDP is open — look at host capture/encode, extra Sunshine on `48010`, or the EGL import error |

A `Couldn't bind RTSP server to port [48010]` line after encoders are found is often a **second bind** on the same Sunshine process (it already owns the port). Smoke tests treat RTSP as OK when `:47990` and `:48010` are the same PID. Fail only when those ports belong to **different** processes, or `:48010` is not listening.

Qt `Failed to register with host portal` on tray start is not a video failure.
