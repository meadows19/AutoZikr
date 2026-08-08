# AutoZikr — Native Windows Zikr Reminders

**AutoZikr** is a lightweight, hardware-accelerated, native Windows desktop application designed to play periodic Zikr reminders. Built in Rust with Direct2D and Win32 APIs, it runs as a 100% self-contained standalone executable with zero external runtime dependencies.

---

## 🌟 Key Features

* **Truly Standalone Executable**: All 12 authentic Zikr audio files are embedded directly inside `autozikr.exe`. Audio streams directly out of RAM memory via Win32 `PlaySoundW` (`SND_MEMORY`) with **0ms latency, zero disk I/O, and zero external files required**.
* **Optional Custom Audio**: Supports loading external `.wav` files placed in a `zikr_audio/` folder alongside `autozikr.exe` if desired.
* **Modern Direct2D GUI**: Hardware-accelerated Direct2D and DirectWrite user interface featuring cards, stepped volume sliders (5% increments), frequency interval sliders (5-minute steps), smooth mouse wheel scrolling, and drag-and-drop thumb controls.
* **Dynamic Light/Dark Theme Sensing**: Senses Windows taskbar themes (`SystemUsesLightTheme`) and automatically switches system tray icons between pure white (dark mode) and deep charcoal (light mode).
* **Past-the-Hour Wall-Clock Alignment**: Reminders automatically align to exact past-the-hour clock boundaries (e.g., 00:05, 00:10, 00:15) regardless of when turned on.
* **Quiet Hours Scheduling**: Configurable Quiet Hours rules supporting presets (`Every Day`, `Work Days`, `Weekends`, `Custom`), custom day selection, and overnight midnight-span rules.
* **Background Audio Sensing**: Queries Windows WASAPI audio meters to prevent playing reminders if media or call audio is already playing on the system.
* **Instant Single-Click Activation**: System tray icon responds instantly on the very first single click to show or hide the control panel.

---

## 📁 Embedded Zikr Audio Library

The single executable binary includes the following 12 authentic Zikr audio files embedded directly at compile time:

1. `4th Kalima.wav`
2. `Alhamdulillah.wav`
3. `Allah Allah.wav`
4. `Allah.wav`
5. `Allahu Akbar.wav`
6. `Hawqala.wav`
7. `Istigfar.wav`
8. `Kalima.wav`
9. `Kalima2.wav`
10. `Kalimatan.wav`
11. `Salawat.wav`
12. `SubhanAllah.wav`

---

## ⚙️ Configuration & Registry Footprint

### Local Configuration File (`config.ini`)
User preferences are persisted locally in `config.ini` in the application directory:

| Key | Values | Description |
| :--- | :--- | :--- |
| `enabled` | `true` / `false` | Master Active / Inactive toggle state |
| `interval_mins` | `5` to `60` (5-min steps) | Reminder frequency interval |
| `volume` | `0` to `100` (5% steps) | Audio playback volume percentage |
| `quiet_hours_enabled` | `true` / `false` | Master Quiet Hours toggle |
| `quiet_hours_rules` | Encoded String | Quiet Hours schedule rules and day masks |
| `run_at_startup` | `true` / `false` | Windows startup preference flag |

### Windows Registry
AutoZikr maintains a clean, minimal registry footprint:

* **Written**: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` -> `AutoZikr` (`REG_SZ`)  
  *Added only when "Launch on Windows Startup" is enabled; removed when disabled.*
* **Read**: `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`  
  *Sensed dynamically to adapt tray icon colors to light or dark taskbars.*

---

## 🛠️ Building from Source

### Prerequisites
* **Rust Toolchain**: `x86_64-pc-windows-msvc`
* **C++ Build Tools**: Visual Studio Build Tools (for `rc.exe`)

### Asset & Release Compilation Commands

```powershell
# 1. (Optional) Generate multi-size star ICO assets (16x16, 24x24, 32x32, 48x48, 256x256)
powershell -ExecutionPolicy Bypass -File "scratch/generate_simple_star_icons.ps1"

# 2. Clean build cache to ensure winres updates app.rc resources
cargo clean

# 3. Compile standalone release binary
cargo build --release
```

The output standalone binary is generated at:
`target\release\autozikr.exe` (~26.2 MB with all 12 WAV files embedded).

---

## 🚀 Deployment Instructions (`C:\Program Files`)

Because **AutoZikr** is a single self-contained executable, deployment requires copying only the executable file:

1. Create directory: `C:\Program Files\AutoZikr\`
2. Copy `target\release\autozikr.exe` into `C:\Program Files\AutoZikr\autozikr.exe`
3. Double-click `autozikr.exe` to run.
