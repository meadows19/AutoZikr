use std::path::Path;
use std::fs;
use std::process::Command;
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;

/// Senses if macOS is currently using Dark Mode via AppleInterfaceStyle defaults.
pub fn is_system_dark_mode() -> bool {
    let output = Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output();
    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout.trim() == "Dark"
    } else {
        false
    }
}

/// Plays embedded WAV audio bytes using native macOS CoreAudio via rodio.
pub fn play_sound_bytes(data: &'static [u8], volume: u32) {
    let vol_factor = ((volume as f32 / 100.0).clamp(0.0, 1.0)).powf(1.4);
    let cursor = Cursor::new(data);

    std::thread::spawn(move || {
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                if let Ok(source) = Decoder::new(cursor) {
                    sink.set_volume(vol_factor);
                    sink.append(source);
                    sink.sleep_until_end();
                }
            }
        }
    });
}

/// Plays external WAV audio from disk with volume scaling via rodio CoreAudio.
pub fn play_sound(path: &Path, volume: u32) {
    if let Ok(bytes) = fs::read(path) {
        let vol_factor = ((volume as f32 / 100.0).clamp(0.0, 1.0)).powf(1.4);
        let cursor = Cursor::new(bytes);

        std::thread::spawn(move || {
            if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&stream_handle) {
                    if let Ok(source) = Decoder::new(cursor) {
                        sink.set_volume(vol_factor);
                        sink.append(source);
                        sink.sleep_until_end();
                    }
                }
            }
        });
    }
}

/// Checks if audio is playing in the background on macOS.
pub fn is_audio_playing() -> bool {
    // macOS CoreAudio detection stub
    false
}

/// Configures macOS LaunchAgent for auto-start on user login.
pub fn set_run_at_startup(enabled: bool) -> Result<(), String> {
    let mut home = match std::env::var("HOME") {
        Ok(h) => std::path::PathBuf::from(h),
        Err(_) => return Err("Could not determine HOME directory".to_string()),
    };
    home.push("Library");
    home.push("LaunchAgents");
    let plist_path = home.join("com.meadows19.autozikr.plist");

    if enabled {
        let exe_path = std::env::current_exe().unwrap_or_default();
        let plist_content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \t<key>Label</key>\n\
             \t<string>com.meadows19.autozikr</string>\n\
             \t<key>ProgramArguments</key>\n\
             \t<array>\n\
             \t\t<string>{}</string>\n\
             \t</array>\n\
             \t<key>RunAtLoad</key>\n\
             \t<true/>\n\
             </dict>\n\
             </plist>",
            exe_path.display()
        );
        let _ = fs::create_dir_all(&home);
        fs::write(plist_path, plist_content).map_err(|e| e.to_string())
    } else {
        if plist_path.exists() {
            let _ = fs::remove_file(plist_path);
        }
        Ok(())
    }
}

pub struct SingleInstanceHandle {
    _file: fs::File,
}

/// Prevents duplicate instances of AutoZikr running on macOS.
pub fn check_single_instance() -> Result<SingleInstanceHandle, ()> {
    let lock_path = match std::env::var("HOME") {
        Ok(h) => format!("{}/.autozikr.lock", h),
        Err(_) => "/tmp/autozikr.lock".to_string(),
    };

    if let Ok(file) = fs::File::create(&lock_path) {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        let res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if res == 0 {
            return Ok(SingleInstanceHandle { _file: file });
        }
    }

    let _ = Command::new("osascript")
        .arg("-e")
        .arg("display notification \"AutoZikr is already running in your Menu Bar.\" with title \"AutoZikr\"")
        .status();

    Err(())
}
