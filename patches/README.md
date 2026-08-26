# Sidecar patches

Eclipse does not fork Sunshine or Moonlight. Patches live here and are applied
by `scripts/build-sidecars.sh` / `build-sidecars.ps1` against the SHAs in
[`vendor/PINS`](../vendor/PINS).

## Apply order

Glob order is apply order:

1. `sunshine-cgnat-pair.patch` — loopback/CGNAT pairing APIs without Basic Auth
2. `sunshine-displays.patch` — `GET /api/displays`, `displayIndex` capture
3. `sunshine-egl-bad-match.patch` — EGL `00003009` DMA-BUF fallback
4. `sunshine-idle-awake.patch` — Windows service idle-sleep inhibit + resume restart
5. `sunshine-sessions.patch` — `GET /api/sessions`, terminate on apps/close
6. `sunshine-udev-prefix.patch` — CMake udev/systemd install dirs
7. `moonlight-01-behavior.patch` — stream contract (C++/headers)
8. `moonlight-02-ui.patch` — connecting splash, Eclipse dialogs/toasts, moon icon, qrc

Each file starts with `Subject:` / `Why:` / `Depends-on:`.

## What never goes in a patch

- **TTF fonts.** `assets/fonts/Remboy-Regular.ttf` and
  `JetBrainsMono-Regular.ttf` are copied into
  `vendor/moonlight-qt/app/res/eclipse/` at sidecar build time. The React
  dashboard uses the WOFF2 Remboy face plus `@fontsource/jetbrains-mono`.
- **Windows ICO files.** `scripts/brand-sidecar-icons.py` writes
  `src-tauri/icons/moonlight.ico` and `sunshine.ico` from the pixel SVGs.
  Sidecar builds copy those over `moonlight.ico` / Sunshine's icon path.
- Build products, `sunshine-prefix`, `moonlight-runtime` DLLs.

## Refresh after you edit a vendor tree

Keep the submodule at the pin. Edit files, then:

```bash
./scripts/refresh-sidecar-patches.sh
# Windows:
.\scripts\refresh-sidecar-patches.ps1
```

That regenerates Moonlight 01/02 and Sunshine egl / udev / idle-awake from
`git diff` (plus `git add -N` for new files). **`sunshine-cgnat-pair.patch`,
`sunshine-displays.patch`, and `sunshine-sessions.patch` share files**
(`network.cpp` / `confighttp.cpp`, `rtsp.cpp`); refresh those by hand so stacked
hunks stay ordered. `sunshine-cgnat-pair` must apply on vanilla PINS Sunshine
(authenticate + `network.{h,cpp}` only) so it sorts before the other confighttp
patches.

## Bump a sidecar (Moonlight or Sunshine)

1. `git -C vendor/<engine> fetch` and checkout the new SHA
2. Update the line in `vendor/PINS`
3. `git -C vendor/<engine> reset --hard <sha>` so the tree is clean
4. `git apply --whitespace=nowarn` the series (or `./scripts/check-sidecar-patches.sh`)
5. Fix conflicts in the vendor tree
6. `./scripts/refresh-sidecar-patches.sh`
7. Rebuild sidecars on **Linux and Windows**
8. Smoke: `./scripts/smoke-video.sh loopback` / `.\scripts\smoke-video.ps1 loopback`

Do not check out Moonlight tag `v6.1.0` (FFmpeg 8). CI runs
`scripts/check-pins.sh` and `scripts/check-sidecar-patches.sh`.
