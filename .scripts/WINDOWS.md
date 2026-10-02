# Windows development

Run these commands from the repository directory in PowerShell:

```powershell
# Verify the native Rust application.
powershell -NoProfile -ExecutionPolicy Bypass -File .scripts/windows.ps1 -Task Check

# Run the Rust tests, including automatic selection and language filtering.
powershell -NoProfile -ExecutionPolicy Bypass -File .scripts/windows.ps1 -Task Test

# Start the app with the Vite development server.
powershell -NoProfile -ExecutionPolicy Bypass -File .scripts/windows.ps1 -Task Dev

# Build the production app and configured installers.
powershell -NoProfile -ExecutionPolicy Bypass -File .scripts/windows.ps1 -Task Build

# Build a standalone debug executable without installers or updater signing.
. ./.scripts/windows.ps1 -Task Env
pnpm tauri build --debug --bundles none
```

The standalone debug executable is `src-tauri/target/debug/pot.exe`.

The helper loads the user-level Rust environment variables, finds Visual Studio
Build Tools with `vswhere`, initializes the x64 developer shell, and keeps Cargo
build outputs in `src-tauri/target` on the repository drive. It does not change
the system execution policy.
Compilation uses four jobs by default; pass `-Jobs 2` or another value to change it.

To use the tools directly in the current PowerShell session:

```powershell
. ./.scripts/windows.ps1 -Task Env
cargo check --locked --manifest-path src-tauri/Cargo.toml
pnpm tauri dev
```

Prerequisites are an MSVC Rust toolchain, Visual Studio C++ Build Tools with a
Windows SDK, Node.js, pnpm, and WebView2. See the
[Tauri 1 Windows prerequisites](https://v1.tauri.app/v1/guides/getting-started/prerequisites/).
Install frontend dependencies with `pnpm install --frozen-lockfile` when needed.

Production installer generation can additionally require the project's existing
updater signing configuration. This is separate from compiling the application.
