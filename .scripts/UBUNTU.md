# Ubuntu compilation

This repository uses **Tauri 1** and **WebKitGTK 4.0**. Ubuntu 20.04 and 22.04 x64
supply the required development packages. Ubuntu 24.04 and newer do not supply
`libwebkit2gtk-4.0-dev` in their standard repositories; installing
`libwebkit2gtk-4.1-dev` alone does not satisfy the current Rust dependencies.
Build on Ubuntu 20.04 when targeting an Ubuntu 20.04 desktop. Executables built
on 22.04 may require newer system libraries and are not a substitute for a
20.04 build. A Windows executable cannot run directly on Ubuntu.

Verified on 2026-10-02: native x64 compilation with embedded frontend and shared
library resolution succeeded inside an Ubuntu 20.04 container, using Rust 1.99,
Node.js 24 and pnpm 11.19.0. This was a debug build without installer bundles;
desktop runtime behavior was not tested.
[Build log](https://github.com/pigzz2/pot-desktop/actions/runs/37014034099).

Keep `Cargo.lock`: Wry is locked to 0.24.12, which fixes the missing WebKit
extension-trait imports on Rust 1.94 and newer. The workspace configuration
also declares the build scripts required by esbuild and tesseract.js for pnpm 11.

Install the native dependencies on Ubuntu 20.04 or 22.04:

```bash
sudo apt update
sudo apt install -y build-essential curl file git pkg-config libssl-dev \
  libgtk-3-dev libwebkit2gtk-4.0-dev libayatana-appindicator3-dev \
  librsvg2-dev patchelf libxdo-dev libxcb1-dev libxrandr-dev libdbus-1-dev \
  libxcb-randr0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxtst-dev
```

Install Rust (stable), Node.js 24 and pnpm 11, then run from the project root:

```bash
pnpm install --frozen-lockfile
pnpm tauri build --bundles none
```

The native executable is `src-tauri/target/release/pot`. To create a Debian
installer, use `pnpm tauri build --bundles deb`. For development on a desktop
session, use `pnpm tauri dev`. System OCR additionally needs `tesseract-ocr`
and the language packages you use.

Automatic selection translation currently uses Windows mouse hooks and UI
Automation. Its controls and tray item are hidden on Linux; the platform code
is conditionally compiled and does not prevent building the rest of Pot on
Ubuntu. Linux support for that particular feature needs a separate implementation.

References:

- [Tauri 1 prerequisites](https://v1.tauri.app/v1/guides/getting-started/prerequisites/)
- [Ubuntu WebKitGTK packages](https://packages.ubuntu.com/search?keywords=libwebkit2gtk&searchon=names&suite=all)
