#![windows_subsystem = "windows"] // Hides console window on startup in release builds

mod config;
mod builtin_audio;
mod platform;

#[cfg(target_os = "windows")]
mod audio;
#[cfg(target_os = "windows")]
mod gui;
#[cfg(target_os = "windows")]
mod tray;

use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use std::fs;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::core::{w, PCWSTR};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, RegisterClassW,
    ShowWindow, TranslateMessage, MSG, WNDCLASSW,
    WS_POPUP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, SW_HIDE, SW_SHOW,
    GetWindowLongPtrW, GWLP_USERDATA, AdjustWindowRectEx,
    WINDOW_EX_STYLE, PostQuitMessage, HCURSOR, HICON,
    LoadCursorW, IDC_ARROW, LoadIconW, IDI_APPLICATION
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::InvalidateRect;

use crate::config::AppConfig;
#[cfg(target_os = "windows")]
use crate::gui::{wnd_proc, AppState};
#[cfg(target_os = "windows")]
use crate::tray::{
    add_tray_icon, remove_tray_icon, show_context_menu, update_tray_status,
    ID_TRAY_EXIT, ID_TRAY_OPEN, ID_TRAY_TOGGLE, WM_TRAY_ICON
};

// SystemTime struct and kernel32 binding to get local system time with zero dependencies
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct SystemTime {
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(lpSystemTime: *mut SystemTime);
}

#[cfg(target_os = "windows")]
pub fn get_local_time() -> SystemTime {
    let mut st = SystemTime::default();
    unsafe { GetLocalTime(&mut st) };
    st
}

#[cfg(target_os = "macos")]
pub fn get_local_time() -> SystemTime {
    use std::time::{SystemTime as StdSystemTime, UNIX_EPOCH};
    let now = StdSystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let sec = (now % 60) as u16;
    let min = ((now / 60) % 60) as u16;
    let hour = ((now / 3600) % 24) as u16;
    SystemTime {
        year: 2026,
        month: 8,
        day_of_week: 6,
        day: 8,
        hour,
        minute: min,
        second: sec,
        milliseconds: 0,
    }
}

pub fn get_time_string() -> String {
    let st = get_local_time();
    let hour = st.hour;
    let minute = st.minute;
    let second = st.second;
    let am_pm = if hour >= 12 { "pm" } else { "am" };
    let display_hour = if hour == 0 {
        12
    } else if hour > 12 {
        hour - 12
    } else {
        hour
    };
    format!("{:02}:{:02}:{:02} {}", display_hour, minute, second, am_pm)
}

pub fn get_zikr_audio_dir() -> PathBuf {
    let mut exe_path = std::env::current_exe().unwrap_or_default();
    exe_path.pop(); // get directory containing executable
    let audio_dir = exe_path.join("zikr_audio");
    if !audio_dir.exists() {
        let _ = fs::create_dir_all(&audio_dir);
    }
    audio_dir
}

pub fn get_audio_files() -> Vec<String> {
    let mut files = Vec::new();
    for (name, _) in builtin_audio::BUILTIN_AUDIO_FILES {
        files.push(name.to_string());
    }
    let dir = get_zikr_audio_dir();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("wav") {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    if !files.contains(&filename.to_string()) {
                        files.push(filename.to_string());
                    }
                }
            }
        }
    }
    files.sort();
    files
}

fn parse_time(s: &str) -> Option<(u32, u32)> {
    let (h_str, m_str) = s.split_once(':')?;
    let h = h_str.parse::<u32>().ok()?;
    let m = m_str.parse::<u32>().ok()?;
    if h < 24 && m < 60 {
        Some((h, m))
    } else {
        None
    }
}

pub fn is_in_quiet_hours(config: &AppConfig) -> bool {
    if !config.quiet_hours_enabled {
        return false;
    }
    
    let rules = crate::config::parse_rules(&config.quiet_hours_rules);
    if rules.is_empty() {
        return false;
    }

    let st = get_local_time();
    let day_idx = if st.day_of_week == 0 {
        6
    } else {
        (st.day_of_week - 1) as usize
    };

    let current_minutes = (st.hour as u32) * 60 + (st.minute as u32);

    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if !rule.days[day_idx] {
            continue;
        }

        let start_minutes = rule.start_hour * 60;
        let end_minutes = rule.end_hour * 60;

        if !rule.overnight {
            if current_minutes >= start_minutes && current_minutes < end_minutes {
                return true;
            }
        } else {
            if current_minutes >= start_minutes || current_minutes < end_minutes {
                return true;
            }
        }
    }

    false
}

fn get_random_index(max: usize) -> usize {
    if max == 0 {
        return 0;
    }
    let st = get_local_time();
    // Simple LCG pseudo-random generator seeded with current time
    let seed = (st.milliseconds as usize) * 1000 + (st.second as usize) * 100 + (st.minute as usize);
    let rand = (seed * 1103515245 + 12345) & 0x7fffffff;
    rand % max
}

pub fn get_seconds_until_next_boundary(interval_mins: u32) -> u32 {
    let st = get_local_time();
    let current_seconds_past_hour = (st.minute as u32) * 60 + (st.second as u32);
    let interval_seconds = interval_mins * 60;
    
    // Calculate the next multiple of interval_seconds past the hour
    let next_reminder = ((current_seconds_past_hour / interval_seconds) + 1) * interval_seconds;
    
    let remaining = next_reminder - current_seconds_past_hour;
    if remaining == 0 {
        interval_seconds
    } else {
        remaining
    }
}

pub struct SingleInstanceHandle(#[cfg(target_os = "windows")] windows::Win32::Foundation::HANDLE);

#[cfg(target_os = "windows")]
pub fn check_single_instance() -> Result<SingleInstanceHandle, ()> {
    unsafe {
        use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowW, PostMessageW, MessageBoxW, MB_OK, MB_ICONINFORMATION,
        };

        let mutex_name = w!("Local\\AutoZikrSingleInstanceMutex");
        let mutex = CreateMutexW(None, true, mutex_name);
        if let Ok(handle) = mutex {
            if GetLastError() == ERROR_ALREADY_EXISTS {
                if let Ok(hwnd) = FindWindowW(w!("AutoZikrWindowClass"), None) {
                    if !hwnd.0.is_null() {
                        let _ = PostMessageW(hwnd, crate::tray::WM_ALREADY_RUNNING, WPARAM(0), LPARAM(0));
                    } else {
                        let _ = MessageBoxW(
                            None,
                            w!("AutoZikr is already running in your system tray."),
                            w!("AutoZikr"),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    }
                } else {
                    let _ = MessageBoxW(
                        None,
                        w!("AutoZikr is already running in your system tray."),
                        w!("AutoZikr"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                return Err(());
            }
            return Ok(SingleInstanceHandle(handle));
        }
    }
    Err(())
}

#[cfg(target_os = "windows")]
fn main() {
    let _instance_guard = match check_single_instance() {
        Ok(h) => h,
        Err(_) => return,
    };

    // 1. Initialize portable config path
    let mut exe_dir = std::env::current_exe().unwrap_or_default();
    exe_dir.pop();
    let config_path = exe_dir.join("config.ini");
    let config = AppConfig::load_from_file(&config_path);

    // Initial check of WAV files in zikr_audio
    let audio_files = get_audio_files();

    // 2. Setup initial shared application state
    let remaining = get_seconds_until_next_boundary(config.interval_mins);
    let total = config.interval_mins * 60;
    let quiet_hours_rules = crate::config::parse_rules(&config.quiet_hours_rules);
    let state = Arc::new(Mutex::new(AppState {
        config,
        config_path,
        remaining_seconds: remaining,
        total_seconds: total,
        audio_files,
        current_tab: 0,
        volume_dragging: false,
        interval_dragging: false,
        next_reminder_tick: false,
        is_dirty: false,
        quiet_hours_rules,
        active_rule_index: None,
        dragging_start_hour: false,
        dragging_end_hour: false,
        dashboard_scroll: 0,
        dashboard_dragging: false,
        scrollbar_dragging: false,
        drag_start_y: 0.0,
        drag_start_scroll: 0,
    }));

    // 3. Register window class and create GUI window
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
        let window_class = w!("AutoZikrWindowClass");

        let hicon = unsafe {
            LoadIconW(hinstance, PCWSTR(1 as *const u16)).unwrap_or_else(|_| {
                LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON::default())
            })
        };
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or(HCURSOR::default()) },
            hIcon: hicon,
            lpszClassName: window_class,
            ..Default::default()
        };

        RegisterClassW(&wc);

        let win_width = 420;
        let win_height = 640;

        // Pass pointer to Arc as lpParam to retrieve in WM_CREATE
        let state_raw = Arc::into_raw(Arc::clone(&state));

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            window_class,
            w!("AutoZikr"),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            win_width,
            win_height,
            None,
            None,
            hinstance,
            Some(state_raw as *const _),
        ).expect("Failed to create main window");

        // 4. Create and attach system tray icon
        add_tray_icon(hwnd);

        // 4b. Check first launch notification
        {
            let mut state_guard = state.lock().unwrap();
            if state_guard.config.first_launch {
                state_guard.config.first_launch = false;
                let cfg_path = state_guard.config_path.clone();
                state_guard.config.save_to_file(&cfg_path);

                crate::tray::show_tray_notification(
                    hwnd,
                    "AutoZikr",
                    "AutoZikr is running! Click the ^ arrow near your clock to find the star icon and open settings.",
                );
            }
        }
        update_tray_status(hwnd, state.lock().unwrap().config.enabled);

        // Start hidden in tray
        ShowWindow(hwnd, SW_HIDE);

        // 5. Spawn background timer and logic thread
        let state_thread = Arc::clone(&state);
        let hwnd_raw = hwnd.0 as isize;
        thread::spawn(move || {
            // Initialize COM once for this background thread to avoid repeated init/deinit overhead
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_MULTITHREADED,
                );
            }

            loop {
                thread::sleep(Duration::from_secs(1));
                
                let mut state = state_thread.lock().unwrap();

                // If configuration is dirty, save to config.ini
                if state.is_dirty {
                    state.config.save_to_file(&state.config_path);
                    state.is_dirty = false;
                }

                if state.config.enabled {
                    let in_quiet = is_in_quiet_hours(&state.config);

                    if !in_quiet {
                        if state.remaining_seconds > 0 {
                            state.remaining_seconds -= 1;
                        } else {
                            // Timer expired, trigger playback!
                            // Periodic check of audio directory contents only right before playback to save disk operations
                            state.audio_files = get_audio_files();
                            if !state.audio_files.is_empty() {
                                // Check if system is playing audio in the background
                                if !audio::is_audio_playing() {
                                    // Select random audio file
                                    let rand_idx = get_random_index(state.audio_files.len());
                                    let selected_file = state.audio_files[rand_idx].clone();
                                    
                                    if let Some(bytes) = builtin_audio::get_builtin_bytes(&selected_file) {
                                        audio::play_sound_bytes(bytes, state.config.volume);
                                    } else {
                                        let mut exe_path = std::env::current_exe().unwrap_or_default();
                                        exe_path.pop();
                                        let full_wav_path = exe_path.join("zikr_audio").join(&selected_file);

                                        // Play it
                                        audio::play_sound(&full_wav_path, state.config.volume);
                                    }
                                }
                            }

                            // Reset countdown timer to the next wall-clock boundary
                            state.remaining_seconds = get_seconds_until_next_boundary(state.config.interval_mins);
                            state.total_seconds = state.config.interval_mins * 60;
                        }
                    }

                    // Force GUI repaint only if the window is currently visible (massive idle CPU savings)
                    let thread_hwnd = HWND(hwnd_raw as *mut _);
                    if unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(thread_hwnd).as_bool() } {
                        unsafe { InvalidateRect(thread_hwnd, None, false) };
                    }
                }
            }
        });

        // 6. Main GUI Message Loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    let _instance_guard = match platform::check_single_instance() {
        Ok(h) => h,
        Err(_) => return,
    };
    println!("AutoZikr starting on macOS...");
    platform::run_macos_app();
}
