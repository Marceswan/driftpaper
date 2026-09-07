# Validation and performance notes

## Automated coverage

Run `cargo test --locked -p flux -p flux-desktop`. GPU tests may skip when no adapter
exists; set `DRIFTPAPER_REQUIRE_GPU_TESTS=1` to make missing GPU support a failure.
CI requires these tests on software Vulkan and runs platform unit tests on macOS
and Windows. Releases depend on these checks.

The tests cover:

- Surface-error recovery decisions and zero-sized windows.
- Display identity across reorder, unplug, and replacement.
- Frame pacing and exclusion of paused time.
- FPS parsing, preference migration, invalid values, and atomic file replacement.
- Independent pause reasons.
- Custom palette serialization and preservation through GPU resize.
- Fluid/noise resizing, refreshed bindings, shared program caches, and zero noise channels.
- Batched simulation substeps versus separately submitted steps.
- Pixel comparison of optimized line/endpoint shaders against the original shader fixtures.

`release/scripts/package-macos.sh` creates the ZIP before GitHub artifact transfer,
extracts it into a temporary directory, checks executable permissions, and validates
the Info.plist. CI also exercises this with a deliberately non-executable input file.

## Verification on the development Mac

On 2026-09-07, 19 core tests and 10 desktop tests passed with Metal GPU access required.
The shader pixel comparison tolerance is one 8-bit channel value. Windows code was
cross-checked with `cargo check --target x86_64-pc-windows-gnu -p flux-desktop`.
The app ran on an Apple M3 Pro and the packaged wallpaper rendered successfully.
Native diagnostics reported `ignores_mouse=true, can_focus=false`.

The production website build (`pnpm build:landing`) passed. Both download buttons
point to `https://github.com/Marceswan/driftpaper/releases`, which returned HTTP 200;
neither link depends on a version number or release asset filename.

The UI automation cannot target the non-activating wallpaper window for a physical
right-click (`noWindowsAvailable`), so Finder's resulting context menu is not yet
visually verified. Physical monitor hotplug and lock/unlock also need the checks below.

## Native platform checks

- macOS: right-click, control-click, and two-finger click on empty desktop should
  open Finder's desktop menu; left-click and drag desktop icons should work.
  Preview windows and the menu bar must remain interactive.
- macOS and Windows: connect, disconnect, reorder, and rotate monitors; move between
  Retina/scaled and unscaled screens. Remove all external monitors and reconnect.
- Lock/unlock, sleep/wake, and display-only sleep should pause and resume animation
  without fast-forwarding. Waking a display must not override an inactive session.
- On Windows, also check session reconnect and both Progman/WorkerW shell layouts.
- Change every menu setting, including custom image palettes and FPS; resize or
  reconnect a monitor, quit, and relaunch. Cancel an image picker without changing
  the active theme. Preview should start with the saved appearance.

## Measuring performance

Use the release build, a fixed display layout, identical preferences and a fixed
sampling duration. Record device, resolution, scaling, density, and FPS. Compare
CPU time, GPU time, memory, and wakeups in Activity Monitor/Instruments; use Metal
System Trace for GPU pass timings. Compare 15/30/60 FPS, single/multiple monitors,
and active/locked/display-asleep states. Frame pacing alone does not lower the
fixed 60 Hz fluid simulation rate.

The changes remove per-fragment HSL work, reuse immutable GPU programs, avoid
unchanged grid reallocation, and clear pressure on the GPU instead of allocating
and uploading full pressure textures on the CPU. No percentage speedup or battery
life improvement is claimed without controlled before/after measurements.

Native APIs: [AppKit mouse pass-through](https://developer.apple.com/documentation/appkit/nswindow/ignoresmouseevents),
[Windows display power notifications](https://learn.microsoft.com/en-us/windows/win32/power/power-setting-guids).
