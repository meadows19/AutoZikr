# AutoZikr — Build & Packaging Guide

This document details the architecture, resource compilation pipeline, build commands, and deployment instructions for **AutoZikr Windows**.

---

## 1. Architecture Overview

AutoZikr is built as a lightweight, zero-dependency, native Windows desktop application written in **Rust**.

* **UI Framework**: Hardware-accelerated Direct2D & DirectWrite via `windows-rs` (`Win32` & `Direct2D` API bindings).
* **Audio Subsystem**: Native Windows Multimedia / WASAPI via COM interfaces.
* **System Integration**: Native Windows System Tray API (`Shell_NotifyIconW`), Windows Registry Theme Sensing (`Personalize`), and Auto-Startup registry integration (`Software\Microsoft\Windows\CurrentVersion\Run`).
* **Binary Distribution**: A single, self-contained standalone executable (~310 KB) requiring no external runtimes, DLLs, or installers.

---

## 2. Icon & Asset Pipeline

The application features custom multi-resolution `.ico` assets rendered at build-time using native GDI+ anti-aliased vector rendering.

### Native ICO Generation
Run the PowerShell asset compiler to generate vector multi-resolution `.ico` files containing 5 native sizes (`16x16`, `24x24`, `32x32`, `48x48`, `256x256`):

```powershell
powershell -ExecutionPolicy Bypass -File "scratch/generate_simple_star_icons.ps1"
```

### Resource Compilation (`app.rc`)
The Windows Resource compiler (`winres`) binds these assets into the executable during Cargo compilation:

* **Resource ID 1 (`app_icon_light.ico`)**: High-contrast dark charcoal star (`#141419`). Windows Explorer uses the first resource entry as the primary `.exe` file icon in File Explorer and Desktop shortcuts. It is also loaded dynamically by the system tray in Light Taskbar mode.
* **Resource ID 2 (`app_icon_dark.ico`)**: Pure white star (`#FFFFFF`). Loaded dynamically by the system tray in Dark Taskbar mode.

---

## 3. How to Build

### Prerequisites
* **Rust Toolchain**: `MSVC` target toolchain installed (`x86_64-pc-windows-msvc`).
* **C++ Build Tools**: Visual Studio Build Tools (for `rc.exe` resource compiler).

### Development Build
```powershell
cargo build
```

### Production Release Build
To compile the fully optimized release binary with embedded icon resources:

```powershell
# Optional: Clean build cache to force winres to re-compile app.rc
cargo clean

# Build optimized release binary
cargo build --release
```

The output binary will be generated at:
`target\release\autozikr.exe`

---

## 4. How to Deploy to `C:\Program Files`

Because **AutoZikr** is designed as a single, self-contained standalone executable, you do not need to copy a complex folder structure.

### Deployment Steps:

1. **Create Installation Directory**:
   Create a new folder inside Program Files:
   ```text
   C:\Program Files\AutoZikr
   ```

2. **Copy Executable**:
   Copy **`autozikr.exe`** from `target\release\` into `C:\Program Files\AutoZikr\autozikr.exe`:
   ```powershell
   Copy-Item "target\release\autozikr.exe" "C:\Program Files\AutoZikr\autozikr.exe"
   ```

3. **Launch Application**:
   Run `C:\Program Files\AutoZikr\autozikr.exe`.
   * The app will automatically initialize its tray icon and configuration file (`autozikr_config.json`).
   * Custom audio files can be placed in `C:\Program Files\AutoZikr\zikr_audio\` or in `%APPDATA%\AutoZikr\zikr_audio\`.
