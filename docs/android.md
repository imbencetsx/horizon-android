# Android port

The Android target builds the existing Horizon Rust game as a NativeActivity
shared library. It shares the simulation, server-thread single player, renderer,
resources, menus and controls with desktop. The desktop GPUI launcher remains in
`launcher/`; it is not an Android launcher yet.

## Build a development APK

Install Rust's `aarch64-linux-android` target, Android SDK platform 36, build-tools
36.0.0, an NDK (tested with r29), and a JDK with `keytool`.

```sh
rustup target add aarch64-linux-android
python3 tools/build_android.py
```

Set `ANDROID_HOME` and optionally `ANDROID_NDK_HOME` or use `--sdk` / `--ndk`.
The default `fast` profile is optimized without LTO. `--profile dev` is for
compile/debug work; do not use it to measure mobile performance. The script
builds ARM64, packages the system NativeActivity without Java source, aligns the
APK for 16 KB pages and signs it with a generated **development** key under
`target/android/`. Output: `target/android/horizon-debug.apk`.

This is a debuggable development APK, including when `--profile release` selects
Rust optimization. It is not a production signing or store release pipeline.
Audio and physical gamepad backends are currently excluded from this build.

## Resources and data

The repository contains the engine and generated original code. Prepared
Minecraft resources are deliberately excluded from Git. Android expects the
same complete prepared folder desktop uses, not just a texture pack:

```
<Android app external files>/resources/horizon-resources.json
<Android app external files>/resources/...
<Android app external files>/game/options.txt
<Android app external files>/game/saves/...
```

For the current package this is normally
`/sdcard/Android/data/dev.playhorizon.android/files/`. It falls back to internal
app storage if Android provides no external app directory. Prepare data from
your own installed Java Edition on desktop using `horizon-prepare --prepare`
and transfer the contents of `.local/horizon-client/26.3` to `resources/`.
Never commit the data or include it in a public APK.

There is no Android file-picker/import flow or Microsoft sign-in yet. Without
the prepared folder, the app shows the existing missing-resources screen.
This is a blocker for validating actual gameplay in a clean checkout.

For agent device validation, build with `--agent-sandbox`. Each launch then uses
a fresh timestamped app-private `agent-runs/` directory, including game,
launcher, XDG and desktop/trash/clipboard scratch folders; no player save is
opened. Read-only resources stay in the shared app `resources/` directory. This APK is for tests, not persistent play.

## Touch controls

Landscape play uses a floating movement origin on the lower left, a camera
finger elsewhere, and independent Jump, Use, Mine, Sneak, Run and Pick buttons
on the lower right. The top row has Pause, Chat, Bag, Drop, Swap and previous /
next hotbar slot. Targets and drawing use the same density-aware rectangles.

Menus support a primary pointer and dragging. The game menu toolbar adds Back,
Right (secondary-click mode), Up/Down scrolling and Keys (Android keyboard).
The right-click mode stays selected until tapped again. This exposes inventory
stack splitting and placing single items. Gameplay actions use the existing
key/controller mappings, including player toggle settings.

Cancellation releases owned inputs; focus loss and screen transitions clear
finger ownership. Android suspend drops both active and spare native surfaces,
pauses local play and stops redraw polling. Resume reacquires the surface while
retaining the device and GPU world buffers.

## Performance and validation

First launch uses a 4-chunk render distance, 5-chunk simulation distance, a 60 FPS cap, reduced
particles and no clouds. Saved player settings survive subsequent launches.
Automatic Android world-worker and resource-loader pools are capped at two
threads per pool. These are conservative starting budgets, not measured FPS
claims or a minimum-device guarantee.

Validation layers:

1. Host tests: `cargo test -p horizon-client-input --lib --no-default-features`.
   Cover move/look/mine together, finger ownership, duplicate action fingers,
   cancellation, menu pointer ownership and shared draw/hit-test geometry.
2. Android compile: `python3 tools/build_android.py --agent-sandbox`. Verify the
   APK signature and 16 KB ELF/APK alignment.
3. Device: install on an ARM64 emulator, launch through the device tools, check
   first-run rendering, touch, Home/resume and crash logs. A missing-resource
   screen is startup evidence only, not a gameplay pass.
4. With prepared data: create a fresh server-thread world; move/look/mine/use
   simultaneously; exercise survival and creative inventory, crafting,
   scrolling, chat, keyboard, signs, books, pause, death, save and reload.
5. Physical low-end devices: measure frame-time percentiles, resident memory,
   cold load, chunk generation and 15+ minutes of thermal behavior. Tune render
   resolution, chunk budgets and memory from those measurements. Emulator FPS
   does not establish phone performance.

Remaining delivery work includes a phone-native resource import/authentication
launcher flow, audio, full text input verification, full action coverage tests,
background/process-death save validation, GPU capability fallbacks, dynamic
resolution and device-specific memory/thermal budgets. Keep these as explicit
gates before calling this a playable Android release.

## Local validation record

On 2026-10-31 the ARM64 Android library passed Clippy with warnings denied,
the desktop game binary passed `cargo check --no-default-features`, and all 65
input-layer unit tests passed. The optimized development APK was signed and
installed on the Android 16 ARM64 emulator, with ELF LOAD alignment 0x4000.
Two Antigravity audits independently checked the input tests and port risks.

The emulator Vulkan driver reproducibly crashed inside
`vulkan.ranchu.so` / `ResourceTracker::on_vkQueueSubmit`. To test the GLES path:

```sh
python3 tools/build_android.py --agent-sandbox --graphics-api opengl
```

Prepared runtime data was subsequently generated locally from hash-verified
public Mojang 26.3 artifacts. It remains ignored under `.local/`. After transfer,
the Android app loaded 35,723 block states, 1,658 items and 67 biomes, and rendered
the title panorama. This is real resource loading, not yet a gameplay pass.
The sandbox smoke-world build subsequently opened a fresh world and displayed
the touch HUD. Touch Bag opened the creative inventory and Pause opened the
pause menu. Terrain was absent while entities rendered, so this does not pass
gameplay validation. After Home/reopen the same process remained alive, but the
device automation helper refused touch injection; its documented debug retry
also failed. Device interaction stopped at that point. Post-resume input is not
verified. No playable FPS or thermal-performance claim is established.

For a fresh server-thread creative-world smoke run (sandbox required):

```sh
python3 tools/build_android.py --agent-sandbox --graphics-api opengl --smoke-world
```

Sandbox builds expose the existing authenticated localhost control transport in
`agent-runs/<run>/game/control.json` for read-only diagnostics through an ADB port
forward. Treat the endpoint token as a secret; do not print or commit that file.
Normal builds do not enable the control transport. First-run mobile settings
also disable the keyboard-only tutorial; Android Back is mapped to Escape.

## Huawei P20 Pro startup fix (awaiting delegated verification)

Gemini Medium reproduced startup failure on the physical Android 10 P20 Pro:
the GLES adapter exposes 256 texture-array layers, while Horizon requested
1,291 layers in one array. Android now defaults to 256-layer sprite arrays
through the shared split used by CPU preparation, GPU uploads and shader
constants. The existing eight-array shader path can hold vanilla's sprites in
six arrays. Desktop keeps its 2,048-layer split. A synthetic regression test
covers all six arrays and the partial final array. Gemini owns builds, checks
and physical-device verification; Sol only implements changes.

## Mobile rendering budget and Huawei verification

First-run Android options now derive from the existing Performance visual
settings (opaque leaves, no mipmaps or biome blending, minimal particles and no
entity shadows), with four-chunk rendering, five-chunk simulation and a
60 FPS cap. Existing player options are not overwritten. This follows the
physical-device preset measurements, which changed multiple options together;
those measurements do not establish any one shader feature as the sole cause.

Android GLES can also use the renderer's existing ordered, compute-built index
lists when the adapter supports vertex storage and compute. This reduces section
draw calls while preserving each section's origin and translucent ordering.
Unsupported adapters retain the previous section draw path. For a controlled
comparison with the same mobile options, the APK builder accepts
`--section-batching off`; the default is `auto`. Gemini owns all build, visual,
regression and performance verification of these changes.

On the physical P20 Pro, Gemini measured approximately 39–41 FPS with Vulkan
at native resolution, compared with 13–14 FPS on GLES with the mobile visual
settings. Vulkan reduced section submissions to three indirect calls. These
short settled-world samples do not establish sustained 60 FPS or performance
while exploring new terrain. GLES flat batching was unsupported on this phone
because its storage binding limit is smaller than the renderer's index page.

First-run world resolution now defaults to 50%; the existing postprocessing path
scales the 3D world while keeping the HUD, menus and touch controls at native
resolution. `--resolution-scale 50..100` selects the initial percentage for
controlled phone comparisons. Gemini's short settled samples measured 43.45 FPS
at 100%, 54.03 FPS at 75% and 59.48 FPS at 50%. The initial unattended
five-minute run crossed the default 60-second AFK threshold, which explicitly
caps the game at 30 FPS. A subsequent controlled idle/input experiment confirmed
this cause: the effective cap changed to 30 while idle and immediately returned
to 60 after a look gesture. The drop was not evidence of FIFO deadline misses.
GPU timestamps were unavailable, so resolution sensitivity supports a rendering
cost explanation without isolating fragment shading from memory bandwidth.
Simulation distance now uses the parser's supported minimum of five: the prior
value of four silently reloaded as twelve. Existing player settings are preserved.

Held touch contacts now refresh the input timestamp each event-loop iteration,
as held controller input already does; a steady joystick or mining hold must not
become AFK simply because Android sends no movement events. Benchmark and stats
responses include `effective_fps_limit` at completion to make intentional caps
visible during profiling. Idle and background energy-saving behavior remains.

Gemini then verified the default Vulkan/50% profile on the physical P20 Pro for
ten consecutive 30-second active-input windows: 59.33 FPS overall, with window
averages of 59.13–59.46 FPS. Median frame times were 16.74–16.75 ms; window
1% lows were 41.57–49.05 FPS, so this is near-60 FPS rather than a guarantee
that every frame meets its deadline. The effective limit stayed at 60. GPU
temperatures settled at 42–44°C and Android reported thermal status NONE.
PSS ranged from 960–982 MB during this run; that bounded observation does not
establish the absence of long-term leaks. Dense builds, prolonged exploration,
other resource packs and weaker phones still require separate measurements.

A 70-second stationary touch hold retained the 60 FPS cap. Movement, look,
hotbar selection, inventory/back, pause/back and Home/resume were checked on
the phone; Jump/Use/Mine buttons were dispatched without errors, which does not
cover every interaction with blocks, items or entities. Gemini also passed
seven touch tests, eleven window tests, formatting and the targeted client,
input and renderer Clippy checks. Detailed measurements and the tested sandbox
APK remain in the ignored `.local/android-qa/` folder; the APK is installed on
the test phone. Test APKs use fresh agent data and are not a persistent player
release or the finished Android launcher.
