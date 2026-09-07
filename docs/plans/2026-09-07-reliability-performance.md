# Desktop reliability and performance

Approved scope: implement every improvement from the repository review.

- Recover surface acquisition errors and skip zero-sized/suspended rendering.
- Reconcile displays by stable identity; share compatible devices and pipelines.
- Make GPU resizing update textures and their bindings together.
- Preserve custom palettes explicitly in settings.
- Persist validated FPS, add live menu controls, pause on display sleep/session lock.
- Preserve the fixed simulation timestep; move invariant brightness work out of fragments.
- Split desktop preferences, display management, rendering, and platform integration.
- Write preferences atomically and report persistence failures.
- Package macOS releases with executable permissions preserved and gate releases on tests.
- Add regression tests and GPU smoke validation; document measurement and platform checks.

Verification: core and desktop Rust tests, GPU smoke tests, formatting, macOS build,
Windows cross-check where toolchains permit, packaging round-trip and CI configuration checks.
Publishing to main and the configured release and website pipelines was subsequently
authorized by the user after local validation.
