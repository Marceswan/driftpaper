<p align="center">
  <img width="100%" src="https://assets.sandydoo.me/flux/social-header-2022-07-07.webp" alt="DriftPaper" />

  <h1 align="center">DriftPaper</h1>
  <p align="center"><b>A live wallpaper for macOS inspired by the Drift screensaver.</b></p>
</p>

<br>

## Features

DriftPaper runs as a menu bar app, rendering a beautiful fluid simulation as your desktop wallpaper. All settings are accessible from the menu bar with live preview - no restart required.

### Menu Bar Controls

| Setting | Options |
|---------|---------|
| **Color Scheme** | Original, Plasma, Poolside, Space Grey |
| **Density** | Sparse, Normal, Dense |
| **Noise Strength** | Low, Medium, High, Max |
| **Line Length** | Short, Medium, Long, Extra Long |
| **Line Width** | Thin, Medium, Thick |
| **View Scale** | Compact, Normal, Wide |

Additional options:
- **Launch at Login** - Automatically start DriftPaper when you log in
- **Quit** - Exit the application

### Technical Details

- Renders behind all windows at desktop level
- Click-through enabled - interact with your desktop normally
- Multi-display support - one window per display
- Low power mode - optimized for battery life
- Settings persist across sessions

## Installation

### Download

Download the latest release from the [Releases](https://github.com/Marceswan/driftpaper/releases) page.

### Build from Source

#### macOS

```sh
# Clone the repository
git clone https://github.com/Marceswan/driftpaper.git
cd driftpaper

# Build release
cargo build --release -p flux-desktop

# The binary is at target/release/drift
# Run directly (wallpaper mode is the default)
./target/release/drift
```

To create a proper macOS app bundle, copy the binary to:
```
/Applications/DriftPaper.app/Contents/MacOS/DriftPaper
```

The app automatically enables wallpaper mode when launched from the bundle.

#### Windows

```sh
# Clone the repository
git clone https://github.com/Marceswan/driftpaper.git
cd driftpaper

# Build release
cargo build --release -p flux-desktop

# The binary is at target/release/drift.exe
# Run directly (wallpaper mode is the default)
.\target\release\drift.exe
```

**Note:** On Windows, DriftPaper uses the WorkerW technique to render behind desktop icons. The system tray icon provides the same menu controls as the macOS version.

#### Cross-compilation (macOS to Windows)

```sh
# Install Windows target
rustup target add x86_64-pc-windows-gnu

# Install mingw-w64 (macOS with Homebrew)
brew install mingw-w64

# Build for Windows
cargo build --release -p flux-desktop --target x86_64-pc-windows-gnu
```

## Usage

Simply launch DriftPaper.app. It will:
1. Appear in your menu bar as "Drift"
2. Render the wallpaper on all displays
3. Save your preferences to `~/.config/driftpaper/preferences.json`

### Command Line Options

```sh
drift --help

Options:
      --windowed     Run in normal window mode (not as wallpaper)
      --fps <FPS>    Target frames per second (default: 60)
  -h, --help         Print help
```

By default, DriftPaper runs as a wallpaper. Use `--windowed` to run in a normal window for testing or preview.

## Credits

DriftPaper is built on [Flux](https://github.com/sandydoo/flux) by [Sander Melnikov](https://github.com/sandydoo/) - an open-source tribute to the macOS Drift screensaver.

> "You're the first person I've seen take this much of an interest in how we made Drift and it looks like you nailed it… minus maybe one or two little elements that give it some extra magic. Great work!"
> — anonymous Apple employee

## License

[MIT](LICENSE) © [Sander Melnikov](https://github.com/sandydoo/) (original Flux project)

Desktop app modifications by [Marc Swan](https://github.com/Marceswan/).
