# Changelog

All notable changes to DriftPaper will be documented in this file.

## [1.2.0] - 2025-01-20

### Changed
- **Wallpaper mode is now the default** - No need to specify `--wallpaper` flag
- Added `--windowed` flag for running in normal window mode (for testing/preview)
- Simplified LaunchAgent plist (removed unnecessary `--wallpaper` argument)

## [1.1.0] - 2025-01-20

### Added
- **Windows Support** - Full Windows implementation with system tray
  - WorkerW technique for rendering behind desktop icons
  - Multi-monitor support via EnumDisplayMonitors
  - System tray with all menu controls (same as macOS)
  - Preferences stored in `%APPDATA%\DriftPaper\preferences.json`

### Changed
- Preferences path is now platform-aware (macOS: `~/.config`, Windows: `%APPDATA%`)
- Menu bar/system tray implementation abstracted per platform
- Updated tray-icon to 0.19, muda to 0.15

## [1.0.0] - 2025-01-20

### Added

#### Menu Bar Controls
Full menu bar interface for controlling the wallpaper without restarting:

- **Color Scheme** submenu
  - Original (default)
  - Plasma
  - Poolside
  - Space Grey

- **Density** submenu (controls line spacing)
  - Sparse (grid_spacing: 35)
  - Normal (grid_spacing: 22)
  - Dense (grid_spacing: 15)

- **Noise Strength** submenu
  - Low (0.15)
  - Medium (0.45) - default
  - High (0.75)
  - Max (1.0)

- **Line Length** submenu
  - Short (200)
  - Medium (450) - default
  - Long (700)
  - Extra Long (1000)

- **Line Width** submenu
  - Thin (4)
  - Medium (9) - default
  - Thick (16)

- **View Scale** submenu
  - Compact (1.0)
  - Normal (1.6) - default
  - Wide (2.2)

- **Launch at Login** toggle
  - Creates/removes LaunchAgent plist

- **Quit DriftPaper** option

#### Core Features
- Live wallpaper rendering at desktop level
- Click-through support - interact with desktop normally
- Multi-display support with synchronized animation
- Persistent preferences saved to `~/.config/driftpaper/preferences.json`
- Screen configuration change detection (resolution changes, display add/remove)
- Auto-wallpaper mode when launched from .app bundle

### Technical Details
- Built with Rust using wgpu for GPU-accelerated rendering
- Uses Cocoa/AppKit for native macOS menu bar integration
- Low power adapter preference for battery optimization
- Configurable FPS via command line (default: 60)

### Credits
Based on [Flux](https://github.com/sandydoo/flux) by Sander Melnikov.
