use std::path::Path;
use std::fs;
use std::process::Command;
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::path::PathBuf;

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
    use tray_icon::{TrayIconBuilder, Icon, TrayIconEvent};
    use tao::event_loop::{EventLoopBuilder, ControlFlow};
    use crate::config::AppConfig;

    let mut exe_dir = std::env::current_exe().unwrap_or_default();
    exe_dir.pop();

    // On macOS .app bundles, store config in ~/Library/Application Support/AutoZikr/
    // For non-bundled executables, use the executable's directory (portable mode)
    let config_dir = if exe_dir.to_string_lossy().contains(".app/Contents/MacOS") {
        match std::env::var("HOME") {
            Ok(home) => {
                let dir = PathBuf::from(home).join("Library").join("Application Support").join("AutoZikr");
                let _ = fs::create_dir_all(&dir);
                dir
            }
            Err(_) => exe_dir.clone(),
        }
    } else {
        exe_dir.clone()
    };
    let config_path = config_dir.join("config.ini");
    let mut config = AppConfig::load_from_file(&config_path);

    if config.first_launch {
        config.first_launch = false;
        config.save_to_file(&config_path);
        let _ = Command::new("osascript")
            .arg("-e")
            .arg("display notification \"AutoZikr is running! Click the star icon in your top Menu Bar to open settings.\" with title \"AutoZikr\"")
            .status();
    }

    let event_loop = EventLoopBuilder::new().build();

    let is_dark = is_system_dark_mode();
    let icon_bytes = create_star_rgba_bytes(32, is_dark);
    let icon = Icon::from_rgba(icon_bytes, 32, 32).unwrap();

    let _tray_icon = TrayIconBuilder::new()
        .with_tooltip("AutoZikr")
        .with_icon(icon)
        .build()
        .unwrap();

    let tray_channel = TrayIconEvent::receiver();

    use tao::window::WindowBuilder;
    use tao::dpi::LogicalSize;
    use wry::WebViewBuilder;

    let window = Arc::new(
        WindowBuilder::new()
            .with_title("AutoZikr Control Panel")
            .with_inner_size(LogicalSize::new(420.0, 640.0))
            .with_resizable(false)
            .with_visible(true)
            .build(&event_loop)
            .unwrap()
    );

    let config_arc = Arc::new(Mutex::new(config.clone()));
    let config_timer = Arc::clone(&config_arc);

    let html_content = generate_control_panel_html(&config);

    let config_ipc = Arc::clone(&config_arc);
    let cfg_path_ipc = config_path.clone();
    let window_ipc = Arc::clone(&window);

    let webview = WebViewBuilder::new()
        .with_html(html_content)
        .with_ipc_handler(move |req| {
            let msg = req.body();
            if msg == "quit" {
                std::process::exit(0);
            } else if msg == "hide" {
                window_ipc.set_visible(false);
            } else if msg == "test_audio" {
                let audio_files = crate::get_audio_files();
                if !audio_files.is_empty() {
                    let rand_idx = crate::get_random_index(audio_files.len());
                    let selected = audio_files[rand_idx].clone();
                    let vol = config_ipc.lock().unwrap().volume;
                    if let Some(bytes) = crate::builtin_audio::get_builtin_bytes(&selected) {
                        play_sound_bytes(bytes, vol);
                    } else {
                        let mut exe_path = std::env::current_exe().unwrap_or_default();
                        exe_path.pop();
                        let full_wav_path = exe_path.join("zikr_audio").join(&selected);
                        play_sound(&full_wav_path, vol);
                    }
                }
            } else if msg.starts_with("save:") {
                let body = &msg[5..];
                let mut cfg = config_ipc.lock().unwrap();
                for pair in body.split(';') {
                    if let Some((k, v)) = pair.split_once('=') {
                        match k {
                            "enabled" => cfg.enabled = v.parse().unwrap_or(true),
                            "interval_mins" => cfg.interval_mins = v.parse().unwrap_or(30),
                            "volume" => cfg.volume = v.parse().unwrap_or(80),
                            "quiet_hours_enabled" => cfg.quiet_hours_enabled = v.parse().unwrap_or(false),
                            "run_at_startup" => {
                                let st = v.parse().unwrap_or(false);
                                if cfg.run_at_startup != st {
                                    cfg.run_at_startup = st;
                                    let _ = set_run_at_startup(st);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                cfg.save_to_file(&cfg_path_ipc);
            }
        })
        .build(&window)
        .unwrap();

    // Keep _webview alive for the duration of the app — dropping it destroys the WebView
    let _webview = webview;

    std::thread::spawn(move || {
        let mut cfg = config_timer.lock().unwrap().clone();
        let mut remaining_seconds = crate::get_seconds_until_next_boundary(cfg.interval_mins);

        loop {
            std::thread::sleep(Duration::from_secs(1));
            
            if let Ok(current) = config_timer.lock() {
                cfg = current.clone();
            }

            if remaining_seconds > 0 {
                remaining_seconds -= 1;
            }

            if remaining_seconds == 0 {
                remaining_seconds = cfg.interval_mins * 60;
                
                if cfg.enabled && !crate::is_in_quiet_hours(&cfg) && !is_audio_playing() {
                    // Refresh audio file list right before playback
                    let audio_files = crate::get_audio_files();
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

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(100));

        if let tao::event::Event::WindowEvent { event: tao::event::WindowEvent::CloseRequested, .. } = event {
            window.set_visible(false);
        }

        if let Ok(t_event) = tray_channel.try_recv() {
            if let TrayIconEvent::Click { .. } = t_event {
                let is_vis = window.is_visible();
                if is_vis {
                    window.set_visible(false);
                } else {
                    window.set_visible(true);
                    window.set_focus();
                }
            }
        }
    });
}

fn generate_control_panel_html(config: &crate::config::AppConfig) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
  body {{
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background-color: #0F172A;
    color: #F8FAFC;
    margin: 0;
    padding: 20px;
    user-select: none;
    -webkit-user-select: none;
  }}
  .card {{
    background-color: #1E293B;
    border-radius: 12px;
    padding: 16px;
    margin-bottom: 16px;
    box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1);
  }}
  .header {{
    text-align: center;
    margin-bottom: 20px;
  }}
  .title {{
    font-size: 22px;
    font-weight: 700;
    color: #10B981;
    margin: 0 0 4px 0;
  }}
  .subtitle {{
    font-size: 13px;
    color: #94A3B8;
    margin: 0;
  }}
  .row {{
    display: flex;
    justify-content: space-between;
    align-items: center;
  }}
  .label {{
    font-size: 14px;
    font-weight: 500;
  }}
  .val-badge {{
    font-size: 13px;
    color: #10B981;
    font-weight: 600;
  }}
  input[type="range"] {{
    width: 100%;
    margin-top: 10px;
    accent-color: #10B981;
  }}
  .switch {{
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
  }}
  .switch input {{ opacity: 0; width: 0; height: 0; }}
  .slider {{
    position: absolute; cursor: pointer; top: 0; left: 0; right: 0; bottom: 0;
    background-color: #475569; transition: .3s; border-radius: 24px;
  }}
  .slider:before {{
    position: absolute; content: ""; height: 18px; width: 18px; left: 3px; bottom: 3px;
    background-color: white; transition: .3s; border-radius: 50%;
  }}
  input:checked + .slider {{ background-color: #10B981; }}
  input:checked + .slider:before {{ transform: translateX(20px); }}
  .btn {{
    background-color: #10B981;
    color: white;
    border: none;
    border-radius: 8px;
    padding: 10px 16px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    width: 100%;
  }}
  .btn:hover {{ background-color: #059669; }}
  .btn-sec {{
    background-color: #334155;
    margin-top: 12px;
  }}
  .btn-sec:hover {{ background-color: #475569; }}
</style>
</head>
<body>
  <div class="header">
    <div style="font-size: 36px; margin-bottom: 4px;">☪️</div>
    <h1 class="title">AutoZikr Control Panel</h1>
    <p class="subtitle">Automated Zikr Audio Reminders</p>
  </div>

  <div class="card">
    <div class="row">
      <span class="label">Reminders Active</span>
      <label class="switch">
        <input type="checkbox" id="enabled" {} onchange="update()">
        <span class="slider"></span>
      </label>
    </div>
  </div>

  <div class="card">
    <div class="row">
      <span class="label">Interval Frequency</span>
      <span class="val-badge" id="interval-val">{} Mins</span>
    </div>
    <input type="range" id="interval" min="5" max="120" step="5" value="{}" oninput="document.getElementById('interval-val').innerText = this.value + ' Mins'; update()">
  </div>

  <div class="card">
    <div class="row">
      <span class="label">Audio Volume</span>
      <span class="val-badge" id="vol-val">{}%</span>
    </div>
    <input type="range" id="volume" min="0" max="100" value="{}" oninput="document.getElementById('vol-val').innerText = this.value + '%'; update()">
    <button class="btn btn-sec" onclick="window.ipc.postMessage('test_audio')">🔊 Test Zikr Sound</button>
  </div>

  <div class="card">
    <div class="row">
      <span class="label">Quiet Hours</span>
      <label class="switch">
        <input type="checkbox" id="quiet" {} onchange="update()">
        <span class="slider"></span>
      </label>
    </div>
  </div>

  <div class="card">
    <div class="row">
      <span class="label">Launch on macOS Startup</span>
      <label class="switch">
        <input type="checkbox" id="startup" {} onchange="update()">
        <span class="slider"></span>
      </label>
    </div>
  </div>

  <div style="display: flex; gap: 12px; margin-top: 20px;">
    <button class="btn" style="flex: 1;" onclick="window.ipc.postMessage('hide')">Done</button>
    <button class="btn" style="flex: 1; background-color: #EF4444;" onclick="window.ipc.postMessage('quit')">Quit AutoZikr</button>
  </div>

<script>
  function update() {{
    const enabled = document.getElementById('enabled').checked;
    const interval = document.getElementById('interval').value;
    const volume = document.getElementById('volume').value;
    const quiet = document.getElementById('quiet').checked;
    const startup = document.getElementById('startup').checked;
    
    const payload = `save:enabled=${{enabled}};interval_mins=${{interval}};volume=${{volume}};quiet_hours_enabled=${{quiet}};run_at_startup=${{startup}}`;
    window.ipc.postMessage(payload);
  }}
</script>
</body>
</html>"#,
        if config.enabled { "checked" } else { "" },
        config.interval_mins,
        config.interval_mins,
        config.volume,
        config.volume,
        if config.quiet_hours_enabled { "checked" } else { "" },
        if config.run_at_startup { "checked" } else { "" }
    )
}
