<div align="center">

# AXIOM Browser

**Vertical-tab-first, privacy-native browser for power users**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-2.0-blue.svg)](https://tauri.app/)

[Features](#-features) • [Architecture](#-architecture) • [Getting Started](#-getting-started) • [Building](#-building) • [Contributing](#-contributing)

</div>

---

## 🎯 Overview

**AXIOM** is a modern desktop browser designed for users who spend hours daily browsing and want **clarity, control, and silence** over algorithmic noise. Built with Rust and Tauri, it prioritizes vertical tabs, privacy protection, and workflow organization through sessions.

### Why AXIOM?

- **🗂️ Vertical Tabs First**: Designed for managing dozens of tabs efficiently
- **🔒 Privacy by Default**: EasyList + EasyPrivacy, tracking parameter stripping, third-party cookie blocking
- **📦 Session Management**: Organize your work into isolated, persistent sessions
- **⚡ Native Performance**: Built with Rust for speed and low memory overhead
- **📖 Reader Mode**: Clean, distraction-free reading experience
- **⬇️ Native Downloads**: Full-featured download manager with pause/resume

---

## ✨ Features

### Core Browsing

- **Vertical Tab Bar**: Optimized for managing many tabs simultaneously
- **Multi-Session Support**: Separate workspaces for different contexts (work, research, personal)
- **Smart Address Bar**: Unified search and navigation with command support
- **Tab States**: Freeze inactive tabs, discard to save memory
- **History & Bookmarks**: Full browsing history with search, organized bookmarks

### Privacy & Security

- **Tracking Protection**: Automatic blocking using EasyList + EasyPrivacy filter lists
- **URL Cleaning**: Strips tracking parameters (utm_*, fbclid, gclid, etc.)
- **Cookie Control**: Third-party cookies blocked by default
- **Permission Management**: Granular per-site permissions for camera, microphone, location
- **No Telemetry**: Your browsing data stays on your device

### Productivity

- **Reader Mode**: Extract and display clean article content
- **Download Manager**: Native downloads with pause/resume, progress tracking
- **Keyboard Shortcuts**: Efficient navigation without touching the mouse
- **Session Persistence**: Auto-save sessions, restore on startup
- **Tab Freezing**: Automatically freeze idle tabs to save resources

---

## 🏗️ Architecture

AXIOM follows a **Rust-first architecture** where all state and business logic lives in Rust, and the WebView is purely for rendering UI.

### Project Structure

```
AXIOM/
├── src-tauri/           # Tauri application & IPC commands
│   ├── src/
│   │   ├── commands/    # Tauri command handlers
│   │   ├── state.rs     # Application state management
│   │   └── lib.rs       # Main application entry
│   └── tauri.conf.json  # Tauri configuration
├── crates/              # Modular Rust crates
│   ├── axiom-core/      # Central coordination & browser state
│   ├── axiom-tabs/      # Tab management with state machine
│   ├── axiom-session/   # Session persistence & restoration
│   ├── axiom-navigation/# History, bookmarks, input resolution
│   ├── axiom-privacy/   # Tracking protection & permissions
│   ├── axiom-download/  # Native download manager
│   └── axiom-storage/   # SQLite database abstraction
└── src/                 # Frontend (Vanilla JS/HTML/CSS)
    ├── index.html
    ├── main.js
    └── styles.css
```

### Technology Stack

- **Backend**: Rust 2021 Edition
- **Framework**: Tauri 2.x
- **Database**: SQLite (via rusqlite)
- **Frontend**: Vanilla JavaScript, HTML5, CSS3
- **Async Runtime**: Tokio
- **Serialization**: Serde

### Design Principles

1. **Rust Owns All State**: WebView is stateless, only renders UI
2. **Modular Crates**: Each domain (tabs, sessions, privacy) is isolated
3. **Auto-Save Everything**: Sessions and state persist on every mutation
4. **Privacy First**: No external services, no telemetry, local-only data

---

## 🚀 Getting Started

### Prerequisites

- **Rust** 1.70 or higher ([Install Rust](https://rustup.rs/))
- **Node.js** 18+ (for Tauri CLI)
- **Platform-specific dependencies**:
  - **Windows**: WebView2 (usually pre-installed on Windows 10/11)
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `webkit2gtk`, `libappindicator3`, `librsvg2`

### Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/projectaxiom/axiom.git
   cd axiom/AXIOM
   ```

2. **Install Tauri CLI**
   ```bash
   cargo install tauri-cli --version "^2.0.0"
   ```

3. **Run in development mode**
   ```bash
   cargo tauri dev
   ```

The browser will launch with hot-reload enabled for both Rust and frontend changes.

---

## 🔨 Building

### Development Build

```bash
cd AXIOM
cargo tauri dev
```

### Production Build

```bash
# Build optimized release version
cargo tauri build

# Output locations:
# - Windows: src-tauri/target/release/AXIOM.exe
#            src-tauri/target/release/bundle/msi/AXIOM_0.1.0_x64_en-US.msi
# - macOS:   src-tauri/target/release/bundle/dmg/AXIOM_0.1.0_universal.dmg
# - Linux:   src-tauri/target/release/bundle/deb/axiom_0.1.0_amd64.deb
```

### Platform-Specific Builds

**Windows (MSI Installer)**
```bash
cargo tauri build --bundles msi
```

**macOS (Universal Binary)**
```bash
cargo tauri build --target universal-apple-darwin
```

**Linux (AppImage)**
```bash
cargo tauri build --bundles appimage
```

---

## 🧪 Testing

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p axiom-tabs
cargo test -p axiom-privacy

# Run with logging
RUST_LOG=debug cargo test
```

---

## 📦 Crate Documentation

### `axiom-core`
Central coordination layer. Manages the `Browser` struct that orchestrates all subsystems.

### `axiom-tabs`
Tab lifecycle management with state machine (Active → Frozen → Discarded).

### `axiom-session`
Session persistence, switching, and tab organization. Auto-saves on every mutation.

### `axiom-navigation`
History tracking, bookmark management, and smart input resolution (URL vs search).

### `axiom-privacy`
Tracking protection (EasyList/EasyPrivacy), permission management, URL cleaning.

### `axiom-download`
Native download manager with pause/resume, progress tracking, and file verification.

### `axiom-storage`
SQLite database abstraction with connection pooling and migrations.

---

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork the repository**
2. **Create a feature branch** (`git checkout -b feature/amazing-feature`)
3. **Make your changes** following the code style
4. **Run tests** (`cargo test --workspace`)
5. **Commit your changes** (`git commit -m 'Add amazing feature'`)
6. **Push to the branch** (`git push origin feature/amazing-feature`)
7. **Open a Pull Request**

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Run Clippy and fix warnings (`cargo clippy`)
- Add tests for new functionality
- Update documentation for API changes

### Development Guidelines

- **Security First**: Never expose secrets, validate all inputs
- **Modular Design**: Keep crates focused and independent
- **Test Coverage**: Write tests for business logic
- **Documentation**: Document public APIs with examples

---

## 📄 License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **Tauri Team** for the excellent framework
- **EasyList** and **EasyPrivacy** for filter lists
- **Rust Community** for amazing libraries and tools

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/projectaxiom/axiom/issues)
- **Discussions**: [GitHub Discussions](https://github.com/projectaxiom/axiom/discussions)

---

<div align="center">

**Built with ❤️ using Rust and Tauri**

</div>
