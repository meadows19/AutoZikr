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

fn create_star_rgba_bytes(size: u32, is_dark: bool) -> Vec<u8> {
    let mut bytes = vec![0u8; (size * size * 4) as usize];
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let rout = size as f32 * 0.42;
    let rin = rout * 0.382;

    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let r = if i % 2 == 0 { rout } else { rin };
        let angle_rad = (-90.0 + (i as f32 * 36.0)) * std::f32::consts::PI / 180.0;
        let px = cx + r * angle_rad.cos();
        let py = cy + r * angle_rad.sin();
        points.push((px, py));
    }

    let color_rgb = if is_dark {
        (255u8, 255u8, 255u8)
    } else {
        (30u8, 30u8, 35u8)
    };

    let stroke_width = 1.8f32;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            
            let mut min_dist_sq = f32::MAX;
            for i in 0..10 {
                let p1 = points[i];
                let p2 = points[(i + 1) % 10];
                let dist_sq = dist_to_segment_sq(px, py, p1.0, p1.1, p2.0, p2.1);
                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                }
            }

            let dist = min_dist_sq.sqrt();
            let alpha_factor = if dist <= stroke_width / 2.0 {
                1.0
            } else if dist <= (stroke_width / 2.0 + 1.0) {
                1.0 - (dist - stroke_width / 2.0)
            } else {
                0.0
            };

            if alpha_factor > 0.0 {
                let idx = ((y * size + x) * 4) as usize;
                bytes[idx] = color_rgb.0;
                bytes[idx + 1] = color_rgb.1;
                bytes[idx + 2] = color_rgb.2;
                bytes[idx + 3] = (alpha_factor * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }

    bytes
}

fn dist_to_segment_sq(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let l2 = (x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1);
    if l2 == 0.0 {
        return (px - x1) * (px - x1) + (py - y1) * (py - y1);
    }
    let t = (((px - x1) * (x2 - x1) + (py - y1) * (y2 - y1)) / l2).clamp(0.0, 1.0);
    let proj_x = x1 + t * (x2 - x1);
    let proj_y = y1 + t * (y2 - y1);
    (px - proj_x) * (px - proj_x) + (py - proj_y) * (py - proj_y)
}

pub fn run_macos_app() {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tray_icon::{TrayIconBuilder, Icon};
    use muda::{Menu, MenuItem, PredefinedMenuItem};
    use tao::event_loop::{EventLoopBuilder, ControlFlow};
    use tao::platform::macos::EventLoopBuilderExtMacOS;
    use crate::config::AppConfig;

    let mut exe_dir = std::env::current_exe().unwrap_or_default();
    exe_dir.pop();
    let config_path = exe_dir.join("config.ini");
    let mut config = AppConfig::load_from_file(&config_path);

    if config.first_launch {
        config.first_launch = false;
        config.save_to_file(&config_path);
        let _ = Command::new("osascript")
            .arg("-e")
            .arg("display notification \"AutoZikr is running! Click the star icon in your top Menu Bar to open settings.\" with title \"AutoZikr\"")
            .status();
    }

    let mut event_loop_builder = EventLoopBuilder::new();
    event_loop_builder.with_default_menu(false);
    let event_loop = event_loop_builder.build();

    let menu = Menu::new();
    let item_toggle = MenuItem::new(
        if config.enabled { "Pause Reminders" } else { "Resume Reminders" },
        true,
        None,
    );
    let item_quit = MenuItem::new("Quit AutoZikr", true, None);

    let _ = menu.append(&item_toggle);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item_quit);

    let is_dark = is_system_dark_mode();
    let icon_bytes = create_star_rgba_bytes(32, is_dark);
    let icon = Icon::from_rgba(icon_bytes, 32, 32).unwrap();

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("AutoZikr")
        .with_icon(icon)
        .build()
        .unwrap();

    let config_arc = Arc::new(Mutex::new(config));
    let config_clone = Arc::clone(&config_arc);

    std::thread::spawn(move || {
        let mut audio_files = crate::get_audio_files();
        let mut cfg = config_clone.lock().unwrap().clone();
        let mut remaining_seconds = crate::get_seconds_until_next_boundary(cfg.interval_mins);

        loop {
            std::thread::sleep(Duration::from_secs(1));
            
            if let Ok(current) = config_clone.lock() {
                cfg = current.clone();
            }

            if remaining_seconds > 0 {
                remaining_seconds -= 1;
            }

            if remaining_seconds == 0 {
                remaining_seconds = cfg.interval_mins * 60;
                
                if cfg.enabled && !crate::is_in_quiet_hours(&cfg) {
                    if !audio_files.is_empty() {
                        let rand_idx = crate::get_random_index(audio_files.len());
                        let selected_file = audio_files[rand_idx].clone();
                        if let Some(bytes) = crate::builtin_audio::get_builtin_bytes(&selected_file) {
                            play_sound_bytes(bytes, cfg.volume);
                        } else {
                            let mut exe_path = std::env::current_exe().unwrap_or_default();
                            exe_path.pop();
                            let full_wav_path = exe_path.join("zikr_audio").join(&selected_file);
                            play_sound(&full_wav_path, cfg.volume);
                        }
                    }
                }
            }
        }
    });

    let toggle_id = item_toggle.id().clone();
    let quit_id = item_quit.id().clone();

    let menu_channel = muda::MenuEvent::receiver();

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(100));

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == toggle_id {
                let mut cfg = config_arc.lock().unwrap();
                cfg.enabled = !cfg.enabled;
                cfg.save_to_file(&config_path);
                item_toggle.set_text(if cfg.enabled { "Pause Reminders" } else { "Resume Reminders" });
            } else if event.id == quit_id {
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}
