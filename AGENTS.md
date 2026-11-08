# Project Rules: Switch Extractor Desktop

## Core Architectural Rules
1. **Tauri v2 Standards**:
   - Follow official Tauri v2 APIs exclusively (`#[tauri::command]`, `invoke()`, `@tauri-apps/api/core`).
   - Never use legacy Tauri v1 APIs.
   - Lock down Tauri capabilities and CSP to minimal local app requirements.

2. **Rust & Frontend Separation**:
   - Core Nintendo Switch extraction logic lives in the separate Rust library crate `crates/switch-extractor-core` with dedicated unit tests.
   - The Tauri backend layer (`src-tauri`) only handles window management, settings persistence, and wires commands/events to the UI.
   - Data crossing the boundary is strictly typed using Serde structs and matching TypeScript interfaces.

3. **Concurrency & Thread Safety**:
   - Long-running tasks (inspection, decryption, RomFS/ExeFS extraction, XCI-to-NSP conversion) MUST run on background Tokio/blocking threads.
   - Live progress, file speeds, and log messages stream to the frontend via Tauri events (`app.emit(...)`).
   - The UI thread must NEVER block or freeze.

4. **Error Handling**:
   - Zero `unwrap()` or `expect()` calls in production runtime code paths.
   - Use `thiserror` for library error types and `anyhow` for top-level command dispatch.
   - Surface errors cleanly to the user via toast notifications and detailed log entries.

5. **Single Standalone Executable Requirements**:
   - The frontend is embedded directly into the binary by Tauri.
   - Link the C runtime statically (`crt-static` for the MSVC Windows target) so the app runs out-of-the-box on a fresh PC without requiring Python or Visual C++ redistributables.
   - Windows GUI subsystem (`windows_subsystem = "windows"`) with no black console prompt.
   - Release profile: `opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.

6. **State & Settings**:
   - Settings (custom key paths, default output directory, window size/position) persist in `%APPDATA%\SwitchExtractor\settings.json`.
   - Frontend state managed using Zustand stores.

7. **BKTR Delta & Update Support**:
   - Game update NCAs use `crypt_type == 4` (AES-BKTR) with bucket tree relocation and subsection blocks.
   - RomFS extraction for updates must merge with a base game package on-the-fly without altering either source file on disk.
   - Counter calculations must account for unaligned container file boundaries.

8. **Multi-Slot Extraction (Base + Update + DLCs)**:
   - Extraction engine supports multi-slot workflows: Base Game, Game Update (optional, BKTR delta merge), and DLC packages (multi-package selection).
   - DLC packages extract their respective RomFS sections into structured directories under `out_dir/dlc/`.

9. **Self-Updating Architecture**:
   - GitHub Releases API integration (`https://api.github.com/repos/{owner}/{repo}/releases/latest`).
   - Self-updating on Windows renames the executing binary to `.old`, writes the new executable, and relaunches cleanly without file lock conflicts.

