# More animations and palettes

Add Aurora Curtains, Pool Caustics and Liquid Metal to the existing animation
selector. Use the shared fullscreen renderer and layered noise texture, with no
additional persistent simulation textures. Aurora layers vertical light curtains;
Caustics approximates moving pools of refracted light; Metal shades a noise-driven
surface with soft studio reflections. All support palette, brightness, scale and
speed independently. Keep existing mode IDs stable.

Add Aurora, Ember, Deep Ocean, Rose Quartz and Moonlight six-color palettes.
Preserve persisted palette IDs, particularly Custom Image (4). Drive both native
menus from a common palette catalog and use stable IDs for checkmarks and event
handling. Expose palette selection in CLI previews as well. Update the macOS
custom-image checkmark handler to recognize the submenu without a fixed item count.

Verify all modes on GPU for distinct, visible, changing output, palette changes,
and portrait/wide resizes. Verify palette persistence and legacy IDs. Inspect GPU
previews, run Rust tests, lint and format checks, build macOS and cross-check Windows.
