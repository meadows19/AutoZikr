# 🌟 AutoZikr (Windows & macOS)

**AutoZikr** is a simple, lightweight desktop application that plays periodic Zikr (Islamic remembrance) audio reminders throughout your day. It runs quietly in your system tray / menu bar with zero setup needed.

---

## 🚀 Download AutoZikr

* 🪟 **[Download for Windows (`autozikr.exe`)](https://github.com/meadows19/AutoZikr/releases/latest/download/autozikr.exe)**
* 🍎 **[Download for macOS (`AutoZikr_macOS_aarch64.dmg`)](https://github.com/meadows19/AutoZikr/releases/latest/download/AutoZikr_macOS_aarch64.dmg)**

*No installer or complex setup required — just download and run!*

---

## 📌 How to Use

1. **Download & Run**: Download `autozikr.exe` and double-click it to start.
2. **Find the Icon in Your System Tray**:
   * AutoZikr runs quietly in the system tray down by your Windows clock.
   * **Don't see the star icon?** Click the **`^`** (overflow arrow) near your clock on the taskbar to view hidden icons. You can drag the **⭐ star icon** out onto your taskbar so it's always visible!
3. **Open Control Panel**: Single-click or right-click the star icon to open settings.

---

## ✨ Features

* **100% Standalone**: All 12 authentic Zikr audio recordings are built directly into the app. No extra folders or audio files needed!
* **Easy Reminders**: Choose how often you'd like to hear a reminder (every 5, 10, 15, 30, or 60 minutes). Reminders automatically align with your clock (e.g. 10:00, 10:15).
* **Simple Volume Control**: Adjust the reminder volume smoothly to your preference.
* **Quiet Hours**: Schedule quiet times to pause reminders while sleeping or working.
* **Smart Audio Detection**: Automatically skips playing reminders if you are listening to music, watching a video, or in a voice call.
* **Matches Your Theme**: Automatically switches colors to fit your Windows Light or Dark taskbar theme.

---

## 🎵 How to Add Your Own Custom Audio Files

While AutoZikr comes with 12 built-in Zikr recordings, you can easily add your own custom audio files:

1. **Folder Location**: Create a folder named **`zikr_audio`** in the exact same location where `autozikr.exe` is saved.
2. **File Format**: Save your custom audio clips inside `zikr_audio` in **`.wav`** format (e.g., `MyZikr.wav`).
3. **Automatic Pick-Up**: Whenever a reminder triggers, AutoZikr will automatically scan `zikr_audio` and include your custom `.wav` files in the random audio selection alongside the built-in recordings!

---

## 🎧 How Smart Audio Detection Works

AutoZikr is designed to be respectful of your active computer usage:

* **Real-Time Sound Card Metering**: Every time a reminder timer reaches zero, AutoZikr queries the Windows Core Audio API (WASAPI) to check if your active speakers or headphones are outputting sound.
* **No Interruption**: If sound is detected (such as a YouTube video, Spotify music, a Zoom/Teams call, or a game), AutoZikr **automatically pauses and skips the reminder** so it never plays over your active audio or interrupts calls.

---

## 🔊 Included Zikr Reminders

AutoZikr randomly plays one of 12 authentic Zikr audio recordings:

1. **SubhanAllah**
2. **Alhamdulillah**
3. **Allahu Akbar**
4. **Astaghfirullah**
5. **Kalima** (*La ilaha illallah*)
6. **Kalima 2** (*Ash-hadu an la ilaha illallah...*)
7. **4th Kalima** (*La ilaha illallahu wahdahu la sharika lah...*)
8. **Allah Allah**
9. **Allah**
10. **Hawqala** (*La hawla wa la quwwata illa billah*)
11. **Kalimatan** (*SubhanAllahi wa bihamdihi SubhanAllahil Adheem*)
12. **Salawat** (*Allahumma Salli ala Muhammad*)

---

## 💻 Building from Source

For developers looking to inspect or compile the Rust source code, check out [BUILDING.md](BUILDING.md).
