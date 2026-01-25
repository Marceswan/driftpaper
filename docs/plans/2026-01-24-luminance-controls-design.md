# Luminance Controls Design

**Date:** 2026-01-24
**Status:** Approved

## Overview

Add brightness presets to the menu bar, allowing users to control overall wallpaper luminance.

## Menu Structure

New "Brightness" submenu after "View Scale":

| Preset | Index | Multiplier | Effective Max |
|--------|-------|------------|---------------|
| Dim    | 0     | 0.5        | ~4.5%         |
| Normal | 1     | 1.0        | ~9% (default) |
| Bright | 2     | 2.0        | ~18%          |
| Vivid  | 3     | 3.5        | ~30%          |

## Implementation

### Files Modified

**flux-desktop/src/main.rs:**
- Add `CURRENT_BRIGHTNESS: AtomicU32` global (default: 1 for Normal)
- Add `brightness: u32` to `UserPreferences` struct
- Add `brightness_to_multiplier(brightness: u32) -> f32` function
- Add Brightness submenu in `setup_menu_bar()`
- Multiply user brightness into `brightness_scale` when building settings

### No Shader Changes Required

The existing `brightness_scale` uniform already controls overall brightness. We multiply the user's preference into this value at the Rust level.

## Testing

1. Build and run: `cargo build --release -p flux-desktop && ./target/release/drift`
2. Verify menu shows Brightness submenu with 4 presets
3. Verify each preset visibly changes wallpaper brightness
4. Verify preference persists across restarts
