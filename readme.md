<div align="center">
  <img src="docs/logo.png" width="64" alt="Lounge logo">
  <h1>Lounge</h1>
  <p>A native Home for rooted LG webOS TVs, written in Rust and drawn directly with OpenGL ES.</p>
  <img src="docs/readme/demo.webp" width="640" alt="Lounge on an LG TV: the dock, the Inputs panel and the app grid">
</div>

Lounge replaces LG's Home screen with a dock of apps over a wallpaper, the rest of the apps one
press down and the TV's inputs one press up. It runs as the TV's real Home: the Home button opens
it, a cold boot lands on it, and it resumes in place instead of restarting.

This page explains how it is built: why it stays fast on a TV's small GPU, how it takes the place
of LG's Home without touching the firmware, and how it is tested.

## Why it is fast

Most webOS launchers, LG's included, are web pages running in LG's WebAppMgr (Chromium) or a QML
scene that stays loaded. Every frame then goes through a browser or a scene graph, and the engine
keeps its memory while you watch something else. Lounge is a single native process that talks to
the GPU directly and does nothing when nothing changes.

- **No engine between the UI and the GPU.** Each frame is a flat draw list compiled into batched
  OpenGL ES 2.0 calls through SDL2. There is no DOM, layout engine or JavaScript.
- **It sleeps.** The loop blocks in SDL until there is input or news from webOS. App, input and
  window changes arrive as Luna bus subscriptions on worker threads, which wake the loop through
  one coalesced event. Idle on Home it wakes only when the clock changes.
- **It draws only what changed.** With EGL partial update a frame repaints only its damaged
  region. Opaque tiles are written to the stencil buffer first, so nothing hidden behind them is
  shaded.
- **Its glass costs one pass.** The Liquid Glass material samples a pre-blurred backdrop baked at
  build time, with refraction at the rim, so no blur runs at frame time.
- **It starts from caches.** Linked shader programs are stored on the TV
  (`GL_OES_get_program_binary`), artwork ships ASTC-compressed for the GPU, and decoding runs on
  worker threads. The drawing thread never reads a file or waits on the network.
- **It gives memory back.** Behind an app it frees its textures and returns heap to the system,
  so the app on screen has the memory.

## Architecture

```
            ┌──────────────── lounge (one process) ────────────────┐
 remote ──▶ │ SDL events ─▶ State ─▶ View ─▶ draw list ─▶ OpenGL ES │ ──▶ TV graphics plane
            │      ▲                                                │
            │      │ wake (one coalesced event)                     │
            │ workers: Luna subscriptions · image decode ·          │
            │          weather · phone photo server                 │
            └───────────────────────────────────────────────────────┘
```

- **State** holds what is on screen and what each remote press does. It is plain data with no I/O,
  so behaviour is unit-tested without a TV or a GPU.
- **View** turns state into a draw list each frame. Text and icons are vector outlines
  tessellated on the CPU (lyon) and drawn as triangle meshes with coverage antialiasing, so they
  stay sharp at any size. Each laid-out line of text is cached.
- **Renderer** batches the list by material (paint, image, glass, vector) and presents it with
  VSync. Animation time sits on the display's refresh grid, so every step of a motion is equal
  even when the loop wakes a little early or late.
- **Workers** do everything that can block: Luna calls, decoding, the weather, file writes. Each
  result wakes the loop once.

## Becoming the TV's Home

Nothing is written to LG's firmware. The takeover uses only mounts, Luna services and settings
that webOS itself reads, and every part is removed again when Lounge as Home is turned off.

- **The takeover.** On webOS 5, 6 and 22 to 24 Lounge's executable is bind-mounted over
  `/usr/bin/com.webos.app.home`, so webOS starts Lounge with LG Home's manifest, window type and
  bus role. On webOS 25 and 26, where sam reads a mounted manifest, a staged Home manifest that
  points at Lounge is mounted instead.
- **Resuming, not restarting.** Lounge registers with sam's app lifecycle (`registerApp`), so a
  Home press raises the running instance instead of starting a new one.
- **Cold boot.** webOS opens the last input as soon as its core boot is done, long before
  Homebrew Channel's boot hooks run. Lounge registers a small root service,
  `lounge-early`, in `/var/luna-service2`, which webOS loads at core-boot-done, plus a key filter
  in the compositor. While the TV boots, the filter calls the service, which mounts Lounge and
  stops the Home webOS restored from its snapshot. The filter then opens Home over the first
  input, so Lounge is the first screen after LG's logo. If any part fails, the Homebrew hook
  takes over later in the boot as before.
- **Staying safe.** Three failed starts within a minute hand Home back to LG. Turning Lounge as
  Home off from the remote removes the mount, the service, the filter and the hook. Uninstalling
  Lounge and restarting the TV does the same.

## Photos from a phone

The wallpaper and the profile picture can come from a phone. Only while its QR code is on screen,
a small HTTP server listens on the TV's LAN address behind a random token. The phone's page
frames the photo, with drag and zoom matching what the TV will show, and sends a JPEG. The TV
decodes it again and bakes the wallpaper's shading and glass backdrops itself. The server stops
when the code closes.

## Testing

- **Unit tests** cover state, navigation, the Home takeover (against a model of webOS: mounts,
  processes, sam and configd), the photo pipeline and the phone server.
- **Headless render tests** run every screen in the native renderer under a headless compositor.
  Each has a budget for screen fill, artwork and text memory; a change that exceeds one fails.
- **On the TV**, `tools/tv-bench.sh` measures frame pacing per scenario.

## Compatibility

LG TVs from 2020 on: webOS 5 and 6, then webOS 22 to 26. Lounge picks the takeover method for
each. Daily use is on webOS 22; the others are covered by the tests. Root
with an elevated [Homebrew Channel](https://www.webosbrew.org/) is required.

## Privacy

No telemetry, no accounts and no log files unless `/var/lib/lounge/debug` exists. The only
network requests are for the weather (Open-Meteo, OpenStreetMap). The phone server listens only
while its QR code is on screen.

## Build and install

```sh
nix develop
pnpm install --frozen-lockfile
pnpm sdk
export WEBOS_NDK="$PWD/.build/toolchain/arm-webos-linux-gnueabi_sdk-buildroot"
pnpm run package   # dist/io.github.yashwanthkamireddi.lounge_1.3.0_arm.ipk
pnpm dev           # desktop preview with sample apps
```

Install the package with [webOS Dev Manager](https://github.com/webosbrew/dev-manager-desktop),
open Lounge and choose **Make Home**. To go back, turn off Settings › General › Lounge as Home.
[development.md](development.md) covers the internals in more depth.

## License

MIT. See [license](license). Not affiliated with LG, Apple or any app brand shown.
