# Dunes, Opal, and Rain Glass

The approved additions are three calm wallpaper modes: softly lit sand ridges,
pearlescent mineral layers beneath frosted glass, and droplets gathering and
sliding over a defocused background. Normal motion is deliberately slow; the
existing Slow/Normal/Fast controls multiply each mode's base rate.

Use the shared artwork shader and noise texture rather than adding another fluid
simulation or an external video dependency. Append the three animation IDs to
preserve saved preferences. The common animation catalog supplies both desktop
menus and CLI choices. Palettes, brightness, view scale, frame pacing, display
reconciliation, and sleep behavior continue through the existing renderer.

Dunes uses asymmetric ridges with diffuse lighting and fine sand ripples. Opal
combines broad color interference with a soft mineral body and static frosting.
Rain Glass uses deterministic cells with staggered droplet growth, sliding,
refraction, and highlights. Its time uniform shares the speed-scaled animation
clock, and complete cycles align with the clock's wrap to avoid a periodic jump.

Verification includes CLI acceptance of all three modes and speed tiers, stable
preference IDs, GPU output/evolution/recolor/resize tests, timing at different
frame rates, rendered preview inspection, the native macOS wallpaper harness,
and Windows cross-target compilation. Existing wallpaper and Topography changes
from this task remain part of the release.

After verification, commit and push to main. The repository's release workflow
builds Windows and universal macOS binaries, signs and notarizes macOS, then
creates the next patch tag and GitHub release. Verify the tag points to the
pushed commit and that the release contains the expected downloads.
