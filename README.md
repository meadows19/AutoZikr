# 🌟 AutoZikr (Windows & macOS)

**AutoZikr** is a simple, lightweight desktop application that plays periodic Zikr (Islamic remembrance) audio reminders throughout your day. It runs quietly in your system tray / menu bar with zero setup needed.

---

* 🪟 **[Download for Windows Setup (`AutoZikr_Windows_Setup.exe`)](https://github.com/meadows19/AutoZikr/releases/latest/download/AutoZikr_Windows_Setup.exe)** *(Recommended Installer)*
* 📦 **[Download Windows Portable (`autozikr.exe`)](https://github.com/meadows19/AutoZikr/releases/latest/download/autozikr.exe)** *(Standalone Portable Binary)*
* 🍎 **[Download for macOS (`AutoZikr_macOS_Universal.dmg`)](https://github.com/meadows19/AutoZikr/releases/latest/download/AutoZikr_macOS_Universal.dmg)** *(Apple Silicon & Intel)*

---

## 📌 How to Use

### 🪟 On Windows:
1. **Download & Run**: Download `autozikr.exe` and double-click it to start.
2. **Find the Icon in Your System Tray**:
   * AutoZikr runs quietly down by your Windows clock.
   * **Don't see the star icon?** Click the **`^`** (overflow arrow) near your clock on the taskbar to view hidden icons. You can drag the **⭐ star icon** out onto your taskbar so it's always visible!
3. **Open Control Panel**: Single-click or right-click the star icon to open settings.

---

### 🍎 On macOS:
1. **Download & Install**: Open `AutoZikr_macOS_Universal.dmg` and drag **AutoZikr** to your `Applications` folder.
2. **Find the Icon in Your Menu Bar**:
   * AutoZikr appears as a **⭐ star icon** in your top macOS menu bar near the clock.
3. **Open Settings**: Click the star icon in your menu bar to open settings or exit.

---

## ✨ Features

* **100% Standalone**: All 12 authentic Zikr audio recordings are built directly into the app. No extra folders or audio files needed!
* **Easy Reminders**: Choose how often you'd like to hear a reminder (every 5, 10, 15, 30, or 60 minutes). Reminders automatically align with your clock (e.g. 10:00, 10:15).
* **Simple Volume Control**: Adjust the reminder volume smoothly to your preference.
* **Quiet Hours**: Schedule quiet times to pause reminders while sleeping or working.
* **Matches Your Theme**: Automatically switches colors to fit your Light or Dark system theme.

---

## 🎙️ How to Add Your Own Custom Audio Files

While AutoZikr comes with 12 built-in Zikr recordings, you can easily add your own custom audio files:

1. **Folder Location**: Create a folder named **`zikr_audio`** in the exact same location where the AutoZikr executable is saved.
2. **File Format**: Save your custom audio clips inside `zikr_audio` in **`.wav`** format (e.g., `MyZikr.wav`).
3. **Automatic Pick-Up**: Whenever a reminder triggers, AutoZikr will automatically scan `zikr_audio` and include your custom `.wav` files in the random audio selection alongside the built-in recordings!

---

## 🔊 Included Zikr Reminders

AutoZikr randomly plays one of 12 authentic Zikr audio recordings:

1. **SubhanAllah**
2. **Alhamdulillah**
3. **Allahu Akbar**
4. **Astaghfirullah**
5. **Kalima** (*La ilaha illallah*)
6. **Kalima 2** (*La ilaha illallah*)
7. **4th Kalima** (*La ilaha illallahu wahdahu la sharika lah...*)
8. **Allah Allah**
9. **Allah**
10. **Hawqala** (*La hawla wa la quwwata illa billah*)
11. **Kalimatan** (*SubhanAllahi wa bihamdihi SubhanAllahil Adheem*)
12. **Salawat** (*Allahumma Salli ala Muhammad*)

---

## 💻 Building from Source

For developers looking to inspect or compile the Rust source code, check out [BUILDING.md](BUILDING.md).

