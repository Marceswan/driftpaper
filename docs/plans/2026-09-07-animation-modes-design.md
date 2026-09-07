# Synthetic animation modes

Implement the three recommended modes: Flowing Silk, Ink in Water, and Living
Topography, alongside the existing default Drift. Keep palette, brightness,
noise strength and view scale independent of mode. Add a shared Slow / Normal /
Fast animation speed. Persist mode and speed, with defaults for old preferences.

Silk and topography use a fullscreen shader sampling the existing layered noise
texture. Silk shades warped folds and fine fibers; topography draws antialiased
contours. A fullscreen pass gives continuous surfaces without increasing the
line grid. Ink adds two bounded RGBA16Float dye textures and a fixed-step
advection/injection compute pass driven by the existing fluid velocity.
Allocate the new renderer only for new modes; release dye textures when leaving
Ink. Cache programs through SharedResources. Skip line placement in new modes,
and skip fluid solving for the two noise-only modes. Rebind on simulation resize.

Expose Animation and Animation Speed menus on macOS and Windows, plus CLI preview
options. Do not reset palettes or other controls when changing mode. Preserve
viewport cropping and per-display resources. Keep the website release URL intact.

Verify GPU shader validation, nonempty/distinct/evolving frames, palette changes,
mode transitions, portrait/landscape resize, defaults and preference round trips.
Run Rust tests, formatting, clippy, macOS build and Windows cross-check. Inspect
actual GPU-rendered preview images. This task builds and verifies locally; release
publication can follow the existing signed release workflow.
