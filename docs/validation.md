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
- Animation timing across speed tiers at 15 and 60 FPS, including the slower
  Topography, Dunes, Opal, and Rain Glass base rates; frozen timestamps do not advance motion.
- Stable numeric animation IDs and CLI selection of the new atmospheric modes.

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

Run the native regression harness from a logged-in desktop session:

```sh
# macOS (or set this environment variable in PowerShell on Windows)
DRIFTPAPER_NATIVE_TESTS=1 cargo test --locked -p flux-desktop --test native_wallpaper
```

This creates temporary native windows and exercises the production window policy on
the main thread. On macOS it checks the activation policy after the event loop starts,
desktop level relative to icons, mouse passthrough flags, focus, Hide, Spaces, native
minimize actions, repeated setup, and preservation of preview window behavior. On
Windows it checks Explorer parenting, click-through/taskbar styles, native minimize
commands, repeated setup, and preview isolation. It skips unless explicitly enabled
because headless CI cannot reliably provide WindowServer or an Explorer desktop.

On 2026-09-19, the native macOS harness passed, along with 23 core tests (Metal GPU
required) and 16 desktop tests. Windows production code and the native harness passed
cross-target compilation. A live Windows desktop is still required for the checks below.

- macOS: right-click, control-click, and two-finger click on empty desktop should
  open Finder's desktop menu; left-click and drag desktop icons should work.
  Preview windows and the menu bar must remain interactive.
- macOS: Show Desktop, Mission Control, switch Spaces, Hide/Hide Others, and minimize
  ordinary apps. The wallpaper must remain in place and must not appear in the Dock
  or Cmd-Tab. `--windowed` previews must remain focusable and minimizable.
- macOS and Windows: connect, disconnect, reorder, and rotate monitors; move between
  Retina/scaled and unscaled screens. Remove all external monitors and reconnect.
- Lock/unlock, sleep/wake, and display-only sleep should pause and resume animation
  without fast-forwarding. Waking a display must not override an inactive session.
- On Windows, also check session reconnect and both Progman/WorkerW shell layouts.
- Windows: right-click empty desktop, drag icons, Win-D twice, Win-M, and Task View.
  The wallpaper must remain behind icons, without a taskbar or Alt-Tab entry. Restart
  Explorer and confirm wallpaper recovery; quit from the tray. Check monitors left
  of/above the primary display as well as mixed DPI arrangements.
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
[stationary desktop windows](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/stationary),
[Windows layered-window hit testing](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows),
[Windows parenting and styles](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent),
[Windows display power notifications](https://learn.microsoft.com/en-us/windows/win32/power/power-setting-guids).
