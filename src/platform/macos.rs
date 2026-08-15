use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString, NSUInteger};
use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyAccessory,
    NSEvent, NSMenu, NSMenuItem,
    NSPopover, NSPopoverBehaviorTransient,
    NSScreen, NSStatusBar, NSStatusItem,
};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL};
use objc::{class, msg_send, sel, sel_impl};

use crate::config::{AppConfig, QuietHoursRule};

// --- Native Audio Engine via macOS AppKit NSSound ---

/// Plays embedded WAV audio bytes directly from memory using native macOS NSSound.
pub fn play_sound_bytes(data: &'static [u8], volume: u32) {
    let vol_factor = ((volume as f32 / 100.0).clamp(0.0, 1.0)).powf(1.4);

    std::thread::spawn(move || unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let ns_data: id = msg_send![class!(NSData), dataWithBytes:data.as_ptr() length:data.len()];
        if ns_data != nil {
            let sound: id = msg_send![class!(NSSound), alloc];
            let sound: id = msg_send![sound, initWithData:ns_data];
            if sound != nil {
                let () = msg_send![sound, setVolume:vol_factor];
                let () = msg_send![sound, play];
                // Keep thread alive until sound finishes
                while {
                    let playing: BOOL = msg_send![sound, isPlaying];
                    playing == YES
                } {
                    std::thread::sleep(Duration::from_millis(100));
                }
                let () = msg_send![sound, release];
            }
        }
        let () = msg_send!(pool, drain);
    });
}

/// Plays external WAV audio from disk with volume scaling via native macOS NSSound.
pub fn play_sound(path: &Path, volume: u32) {
    if let Some(path_str) = path.to_str() {
        let path_owned = path_str.to_string();
        let vol_factor = ((volume as f32 / 100.0).clamp(0.0, 1.0)).powf(1.4);

        std::thread::spawn(move || unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let ns_path = NSString::alloc(nil).init_str(&path_owned);
            let sound: id = msg_send![class!(NSSound), alloc];
            let sound: id = msg_send![sound, initWithContentsOfFile:ns_path byReference:NO];
            if sound != nil {
                let () = msg_send![sound, setVolume:vol_factor];
                let () = msg_send![sound, play];
                while {
                    let playing: BOOL = msg_send![sound, isPlaying];
                    playing == YES
                } {
                    std::thread::sleep(Duration::from_millis(100));
                }
                let () = msg_send![sound, release];
            }
            let () = msg_send!(pool, drain);
        });
    }
}

/// Checks if audio is currently playing in the background on macOS using pmset assertions.
pub fn is_audio_playing() -> bool {
    if let Ok(out) = Command::new("pmset")
        .args(["-g", "assertions"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains("PreventUserIdleSystemSleep") && stdout.contains("coreaudiod") {
            return true;
        }
    }
    false
}

/// Checks if the laptop lid is currently closed on macOS via AppleClamshellState.
pub fn is_lid_closed() -> bool {
    let output = Command::new("ioreg")
        .args(["-r", "-k", "AppleClamshellState"])
        .output();
    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout.contains("\"AppleClamshellState\" = Yes")
    } else {
        false
    }
}

/// Senses if macOS is currently using Dark Mode.
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

/// Configures macOS LaunchAgent for auto-start on user login.
pub fn set_run_at_startup(enabled: bool) -> Result<(), String> {
    let mut home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
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

pub fn is_run_at_startup_enabled() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        let plist_path = PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("com.meadows19.autozikr.plist");
        return plist_path.exists();
    }
    false
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

// --- Shared Application State ---

pub struct MacAppState {
    pub config: AppConfig,
    pub config_path: PathBuf,
    pub remaining_seconds: u32,
    pub total_seconds: u32,
    pub audio_files: Vec<String>,
    pub current_tab: u32, // 0: Dashboard, 1: Settings
    pub volume_dragging: bool,
    pub interval_dragging: bool,
    pub is_dirty: bool,

    // Quiet Hours state
    pub quiet_hours_rules: Vec<QuietHoursRule>,
    pub active_rule_index: Option<usize>,
    pub dragging_start_hour: bool,
    pub dragging_end_hour: bool,

    // Scrolling state
    pub dashboard_scroll: f32,
    pub dashboard_dragging: bool,
    pub scrollbar_dragging: bool,
    pub drag_start_y: f32,
    pub drag_start_scroll: f32,
}

static mut GLOBAL_STATE: Option<Arc<Mutex<MacAppState>>> = None;
static mut GLOBAL_VIEW: id = nil;
static mut GLOBAL_POPOVER: id = nil;
static mut GLOBAL_STATUS_ITEM: id = nil;

fn get_state() -> &'static Arc<Mutex<MacAppState>> {
    unsafe { GLOBAL_STATE.as_ref().expect("GLOBAL_STATE not initialized") }
}

// --- CoreGraphics Drawing Helpers ---

fn create_star_template_image(size: f64) -> id {
    unsafe {
        let s = size as u32;
        let mut bytes = vec![0u8; (s * s * 4) as usize];
        let cx = size as f32 / 2.0;
        let cy = size as f32 / 2.0;
        let rout = size as f32 * 0.42;
        let rin = rout * 0.382;

        let mut points = Vec::with_capacity(10);
        for i in 0..10 {
            let r = if i % 2 == 0 { rout } else { rin };
            let angle_rad = (-90.0 + (i as f32 * 36.0)) * std::f32::consts::PI / 180.0;
            points.push((cx + r * angle_rad.cos(), cy + r * angle_rad.sin()));
        }

        let stroke_width = 1.8f32;
        for y in 0..s {
            for x in 0..s {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let mut min_dist_sq = f32::MAX;
                for i in 0..10 {
                    let p1 = points[i];
                    let p2 = points[(i + 1) % 10];
                    let l2 = (p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2);
                    let t = if l2 == 0.0 { 0.0 } else { (((px - p1.0)*(p2.0 - p1.0) + (py - p1.1)*(p2.1 - p1.1)) / l2).clamp(0.0, 1.0) };
                    let proj = (p1.0 + t * (p2.0 - p1.0), p1.1 + t * (p2.1 - p1.1));
                    let d_sq = (px - proj.0).powi(2) + (py - proj.1).powi(2);
                    if d_sq < min_dist_sq { min_dist_sq = d_sq; }
                }
                let dist = min_dist_sq.sqrt();
                let alpha = if dist <= stroke_width / 2.0 { 1.0 } else if dist <= stroke_width / 2.0 + 1.0 { 1.0 - (dist - stroke_width / 2.0) } else { 0.0 };
                if alpha > 0.0 {
                    let idx = ((y * s + x) * 4) as usize;
                    bytes[idx] = 255;
                    bytes[idx + 1] = 255;
                    bytes[idx + 2] = 255;
                    bytes[idx + 3] = (alpha * 255.0).clamp(0.0, 255.0) as u8;
                }
            }
        }

        let planes: [*const u8; 5] = [bytes.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null()];
        let img_rep: id = msg_send![class!(NSBitmapImageRep), alloc];
        let img_rep: id = msg_send![img_rep,
            initWithBitmapDataPlanes:planes.as_ptr()
            pixelsWide:s as isize
            pixelsHigh:s as isize
            bitsPerSample:8 as isize
            samplesPerPixel:4 as isize
            hasAlpha:YES
            isPlanar:NO
            colorSpaceName:NSString::alloc(nil).init_str("NSCalibratedRGBColorSpace")
            bytesPerRow:(s * 4) as isize
            bitsPerPixel:32 as isize
        ];
        if img_rep != nil {
            let img: id = msg_send![class!(NSImage), alloc];
            let img: id = msg_send![img, initWithSize:NSSize { width: 18.0, height: 18.0 }];
            let () = msg_send![img, addRepresentation:img_rep];
            let () = msg_send![img, setTemplate:YES]; // Native macOS template icon
            img
        } else {
            nil
        }
    }
}

// --- Custom Native NSView with CoreGraphics Rendering ---

extern "C" fn view_draw_rect(this: &Object, _cmd: Sel, _dirty_rect: NSRect) {
    unsafe {
        let ctx_obj: id = msg_send![class!(NSGraphicsContext), currentContext];
        if ctx_obj == nil { return; }
        let cg_ctx: CGContextRef = msg_send![ctx_obj, CGContext];
        if cg_ctx.is_null() { return; }

        let state_arc = get_state();
        let state = state_arc.lock().unwrap();
        let is_dark = is_system_dark_mode();

        // Theme palette matching Direct2D GUI
        let (bg_r, bg_g, bg_b) = if is_dark { (15.0/255.0, 15.0/255.0, 19.0/255.0) } else { (240.0/255.0, 240.0/255.0, 245.0/255.0) };
        let (card_r, card_g, card_b) = if is_dark { (27.0/255.0, 27.0/255.0, 34.0/255.0) } else { (255.0/255.0, 255.0/255.0, 255.0/255.0) };
        let (accent_r, accent_g, accent_b) = if is_dark { (177.0/255.0, 159.0/255.0, 251.0/255.0) } else { (120.0/255.0, 90.0/255.0, 230.0/255.0) };
        let (text_r, text_g, text_b) = if is_dark { (1.0, 1.0, 1.0) } else { (30.0/255.0, 30.0/255.0, 40.0/255.0) };
        let (gray_r, gray_g, gray_b) = if is_dark { (142.0/255.0, 140.0/255.0, 154.0/255.0) } else { (110.0/255.0, 110.0/255.0, 125.0/255.0) };
        let (border_r, border_g, border_b) = if is_dark { (44.0/255.0, 44.0/255.0, 53.0/255.0) } else { (215.0/255.0, 215.0/255.0, 225.0/255.0) };

        // 1. Clear background
        let bg_rect = NSRect { origin: NSPoint { x: 0.0, y: 0.0 }, size: NSSize { width: 420.0, height: 640.0 } };
        let bg_color: id = msg_send![class!(NSColor), colorWithRed:bg_r green:bg_g blue:bg_b alpha:1.0];
        let () = msg_send![bg_color, setFill];
        let () = msg_send![class!(NSBezierPath), fillRect:bg_rect];

        // Helper for drawing text in Cocoa coordinate system (Cocoa Y=0 is bottom)
        let draw_cocoa_text = |text: &str, x: f64, y_from_top: f64, font_size: f64, bold: bool, color: (f64, f64, f64), align: u32| {
            let ns_str = NSString::alloc(nil).init_str(text);
            let font_name = if bold { "SFPro-Bold" } else { "SFPro-Regular" };
            let font: id = msg_send![class!(NSFont), fontWithName:NSString::alloc(nil).init_str(font_name) size:font_size];
            let font = if font == nil { msg_send![class!(NSFont), systemFontOfSize:font_size] } else { font };
            let ns_color: id = msg_send![class!(NSColor), colorWithRed:color.0 green:color.1 blue:color.2 alpha:1.0];

            let dict: id = msg_send![class!(NSMutableDictionary), dictionary];
            let () = msg_send![dict, setObject:font forKey:NSString::alloc(nil).init_str("NSFont")];
            let () = msg_send![dict, setObject:ns_color forKey:NSString::alloc(nil).init_str("NSColor")];

            let str_size: NSSize = msg_send![ns_str, sizeWithAttributes:dict];
            let cocoa_y = 640.0 - y_from_top - str_size.height;
            let draw_x = match align {
                1 => x - (str_size.width / 2.0), // Center
                2 => x - str_size.width,          // Right
                _ => x,                           // Left
            };
            let () = msg_send![ns_str, drawAtPoint:NSPoint { x: draw_x, y: cocoa_y } withAttributes:dict];
        };

        // Draw rounded rectangle helper
        let draw_rounded_card = |x: f64, y_from_top: f64, w: f64, h: f64, radius: f64, fill: (f64, f64, f64), border: (f64, f64, f64)| {
            let cocoa_y = 640.0 - y_from_top - h;
            let rect = NSRect { origin: NSPoint { x, y: cocoa_y }, size: NSSize { width: w, height: h } };
            let path: id = msg_send![class!(NSBezierPath), bezierPathWithRoundedRect:rect xRadius:radius yRadius:radius];
            let fill_color: id = msg_send![class!(NSColor), colorWithRed:fill.0 green:fill.1 blue:fill.2 alpha:1.0];
            let () = msg_send![fill_color, setFill];
            let () = msg_send![path, fill];

            let border_color: id = msg_send![class!(NSColor), colorWithRed:border.0 green:border.1 blue:border.2 alpha:1.0];
            let () = msg_send![border_color, setStroke];
            let () = msg_send![path, setLineWidth:1.0];
            let () = msg_send![path, stroke];
        };

        // Draw line helper
        let draw_line = |x1: f64, y1_top: f64, x2: f64, y2_top: f64, width: f64, color: (f64, f64, f64)| {
            let p1 = NSPoint { x: x1, y: 640.0 - y1_top };
            let p2 = NSPoint { x: x2, y: 640.0 - y2_top };
            let path: id = msg_send![class!(NSBezierPath), bezierPath];
            let () = msg_send![path, moveToPoint:p1];
            let () = msg_send![path, lineToPoint:p2];
            let () = msg_send![path, setLineWidth:width];
            let ns_color: id = msg_send![class!(NSColor), colorWithRed:color.0 green:color.1 blue:color.2 alpha:1.0];
            let () = msg_send![ns_color, setStroke];
            let () = msg_send![path, stroke];
        };

        // Draw Toggle Switch helper
        let draw_toggle = |x: f64, y_from_top: f64, enabled: bool| {
            let w = 48.0; let h = 26.0;
            let cocoa_y = 640.0 - y_from_top - h;
            let rect = NSRect { origin: NSPoint { x, y: cocoa_y }, size: NSSize { width: w, height: h } };
            let path: id = msg_send![class!(NSBezierPath), bezierPathWithRoundedRect:rect xRadius:13.0 yRadius:13.0];
            if enabled {
                let color: id = msg_send![class!(NSColor), colorWithRed:accent_r green:accent_g blue:accent_b alpha:1.0];
                let () = msg_send![color, setFill];
                let () = msg_send![path, fill];
                let thumb_rect = NSRect { origin: NSPoint { x: x + w - 23.0, y: cocoa_y + 3.0 }, size: NSSize { width: 20.0, height: 20.0 } };
                let thumb: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:thumb_rect];
                let white: id = msg_send![class!(NSColor), whiteColor];
                let () = msg_send![white, setFill];
                let () = msg_send![thumb, fill];
            } else {
                let color: id = msg_send![class!(NSColor), colorWithRed:border_r green:border_g blue:border_b alpha:1.0];
                let () = msg_send![color, setFill];
                let () = msg_send![path, fill];
                let thumb_rect = NSRect { origin: NSPoint { x: x + 3.0, y: cocoa_y + 3.0 }, size: NSSize { width: 20.0, height: 20.0 } };
                let thumb: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:thumb_rect];
                let white: id = msg_send![class!(NSColor), whiteColor];
                let () = msg_send![white, setFill];
                let () = msg_send![thumb, fill];
            }
        };

        // --- Header (Y: 20 to 75) ---
        draw_cocoa_text("AutoZikr", 20.0, 25.0, 20.0, true, (text_r, text_g, text_b), 0);
        draw_cocoa_text("Remember to Remember", 20.0, 52.0, 12.0, false, (gray_r, gray_g, gray_b), 0);
        draw_cocoa_text("Active", 320.0, 35.0, 13.0, true, if state.config.enabled { (text_r, text_g, text_b) } else { (gray_r, gray_g, gray_b) }, 2);
        draw_toggle(330.0, 28.0, state.config.enabled);

        // --- Active Tab Content ---
        if state.current_tab == 0 {
            // Dashboard View
            let scroll_y = state.dashboard_scroll;

            // 1. Timer Card (Y: 85 to 285)
            draw_rounded_card(20.0, 85.0 - scroll_y as f64, 380.0, 195.0, 12.0, (card_r, card_g, card_b), (border_r, border_g, border_b));

            let cx = 210.0;
            let cy_top = 182.0 - scroll_y as f64;
            let cocoa_cy = 640.0 - cy_top;
            let radius = 62.0;

            if state.config.enabled {
                // Background Track Circle
                let ring_rect = NSRect { origin: NSPoint { x: cx - radius, y: cocoa_cy - radius }, size: NSSize { width: radius * 2.0, height: radius * 2.0 } };
                let track_path: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:ring_rect];
                let () = msg_send![track_path, setLineWidth:8.0];
                let border_c: id = msg_send![class!(NSColor), colorWithRed:border_r green:border_g blue:border_b alpha:1.0];
                let () = msg_send![border_c, setStroke];
                let () = msg_send![track_path, stroke];

                // Progress Arc
                let in_quiet = crate::is_in_quiet_hours(&state.config);
                let (ring_r, ring_g, ring_b) = if in_quiet { (border_r, border_g, border_b) } else { (accent_r, accent_g, accent_b) };
                let pct = if state.total_seconds > 0 { (state.remaining_seconds as f64 / state.total_seconds as f64).clamp(0.0, 1.0) } else { 0.0 };

                if pct > 0.0 {
                    let arc_path: id = msg_send![class!(NSBezierPath), bezierPath];
                    let start_deg = 90.0;
                    let end_deg = 90.0 - (pct * 360.0);
                    let () = msg_send![arc_path, appendBezierPathWithArcWithCenter:NSPoint { x: cx, y: cocoa_cy } radius:radius startAngle:start_deg endAngle:end_deg clockwise:YES];
                    let () = msg_send![arc_path, setLineWidth:8.0];
                    let () = msg_send![arc_path, setLineCapStyle:1 as NSUInteger]; // NSRoundLineCapStyle
                    let ring_c: id = msg_send![class!(NSColor), colorWithRed:ring_r green:ring_g blue:ring_b alpha:1.0];
                    let () = msg_send![ring_c, setStroke];
                    let () = msg_send![arc_path, stroke];
                }

                // Digits & Label
                let mins = state.remaining_seconds / 60;
                let secs = state.remaining_seconds % 60;
                let time_str = format!("{:02}:{:02}", mins, secs);
                draw_cocoa_text(&time_str, cx, cy_top - 18.0, 32.0, true, (text_r, text_g, text_b), 1);
                let label = if in_quiet { "QUIET HOURS" } else { "NEXT REMINDER" };
                draw_cocoa_text(label, cx, cy_top + 20.0, 10.0, true, (gray_r, gray_g, gray_b), 1);
            } else {
                draw_cocoa_text("Reminders are Off", cx, cy_top - 12.0, 16.0, true, (text_r, text_g, text_b), 1);
                draw_cocoa_text("Toggle 'Active' at top to start timer.", cx, cy_top + 12.0, 12.0, false, (gray_r, gray_g, gray_b), 1);
            }

            // 2. Quiet Hours Card (Y: 295+)
            let qh_y = 295.0 - scroll_y as f64;
            let qh_h = if state.config.quiet_hours_enabled { 80.0 + (state.quiet_hours_rules.len() as f64 * 195.0) + 48.0 } else { 80.0 };
            draw_rounded_card(20.0, qh_y, 380.0, qh_h, 12.0, (card_r, card_g, card_b), (border_r, border_g, border_b));

            draw_cocoa_text("Quiet Hours", 40.0, qh_y + 16.0, 14.0, true, (text_r, text_g, text_b), 0);
            draw_cocoa_text("Silence reminders during custom times", 40.0, qh_y + 36.0, 12.0, false, (gray_r, gray_g, gray_b), 0);
            draw_toggle(330.0, qh_y + 16.0, state.config.quiet_hours_enabled);

            if state.config.quiet_hours_enabled {
                let mut rule_y = qh_y + 70.0;
                for (idx, rule) in state.quiet_hours_rules.iter().enumerate() {
                    draw_rounded_card(35.0, rule_y, 350.0, 180.0, 8.0, (bg_r, bg_g, bg_b), (border_r, border_g, border_b));

                    // Rule Enabled toggle
                    draw_toggle(50.0, rule_y + 12.0, rule.enabled);

                    // Preset Badge
                    let preset_str = match rule.preset.as_str() {
                        "every_day" => "Every Day ▼",
                        "work_days" => "Work Days ▼",
                        "weekends" => "Weekends ▼",
                        _ => "Custom ▼",
                    };
                    draw_rounded_card(110.0, rule_y + 12.0, 100.0, 26.0, 6.0, (card_r, card_g, card_b), (border_r, border_g, border_b));
                    draw_cocoa_text(preset_str, 160.0, rule_y + 17.0, 11.0, true, (text_r, text_g, text_b), 1);

                    // Delete button (✕)
                    draw_cocoa_text("✕", 360.0, rule_y + 14.0, 14.0, true, (gray_r, gray_g, gray_b), 1);

                    // Time summary
                    let summary = format!("Mute: {:02}:00 - {:02}:00{}", rule.start_hour, rule.end_hour, if rule.overnight { " (Overnight)" } else { "" });
                    draw_cocoa_text(&summary, 50.0, rule_y + 46.0, 12.0, true, (accent_r, accent_g, accent_b), 0);

                    // Day Bubbles (M T W T F S S)
                    let day_labels = ["M", "T", "W", "T", "F", "S", "S"];
                    for d in 0..7 {
                        let cx_day = 55.0 + (d as f64 * 42.0);
                        let cy_day = rule_y + 82.0;
                        let cocoa_day_y = 640.0 - cy_day;
                        let is_active = rule.days[d];
                        let (db_r, db_g, db_b) = if is_active { (accent_r, accent_g, accent_b) } else { (border_r, border_g, border_b) };
                        let dot_rect = NSRect { origin: NSPoint { x: cx_day - 13.0, y: cocoa_day_y - 13.0 }, size: NSSize { width: 26.0, height: 26.0 } };
                        let dot: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:dot_rect];
                        let dot_color: id = msg_send![class!(NSColor), colorWithRed:db_r green:db_g blue:db_b alpha:1.0];
                        let () = msg_send![dot_color, setFill];
                        let () = msg_send![dot, fill];
                        draw_cocoa_text(day_labels[d], cx_day, cy_day - 7.0, 11.0, true, if is_active { (1.0, 1.0, 1.0) } else { (gray_r, gray_g, gray_b) }, 1);
                    }

                    // Double Range Slider
                    let track_top = rule_y + 125.0;
                    draw_line(50.0, track_top, 350.0, track_top, 6.0, (border_r, border_g, border_b));

                    let x_start = 50.0 + (rule.start_hour as f64 / 23.0) * 300.0;
                    let x_end = 50.0 + (rule.end_hour as f64 / 23.0) * 300.0;

                    if !rule.overnight {
                        draw_line(x_start, track_top, x_end, track_top, 6.0, (accent_r, accent_g, accent_b));
                    } else {
                        draw_line(50.0, track_top, x_end, track_top, 6.0, (accent_r, accent_g, accent_b));
                        draw_line(x_start, track_top, 350.0, track_top, 6.0, (accent_r, accent_g, accent_b));
                    }

                    // Thumbs
                    for thumb_x in [x_start, x_end] {
                        let t_rect = NSRect { origin: NSPoint { x: thumb_x - 4.0, y: (640.0 - track_top) - 10.0 }, size: NSSize { width: 8.0, height: 20.0 } };
                        let thumb: id = msg_send![class!(NSBezierPath), bezierPathWithRoundedRect:t_rect xRadius:4.0 yRadius:4.0];
                        let white: id = msg_send![class!(NSColor), whiteColor];
                        let () = msg_send![white, setFill];
                        let () = msg_send![thumb, fill];
                    }

                    // Overnight toggle row
                    draw_cocoa_text("Overnight (Spans Midnight)", 50.0, rule_y + 150.0, 12.0, false, (gray_r, gray_g, gray_b), 0);
                    draw_toggle(325.0, rule_y + 146.0, rule.overnight);

                    rule_y += 195.0;
                }

                // Add Rule Button
                draw_rounded_card(35.0, rule_y, 350.0, 36.0, 8.0, (bg_r, bg_g, bg_b), (border_r, border_g, border_b));
                draw_cocoa_text("+ Add Rule", 210.0, rule_y + 10.0, 13.0, true, (accent_r, accent_g, accent_b), 1);
            }
        } else {
            // Settings Tab
            // 1. Volume Card
            draw_rounded_card(20.0, 90.0, 380.0, 90.0, 12.0, (card_r, card_g, card_b), (border_r, border_g, border_b));
            draw_cocoa_text("REMINDER VOLUME", 40.0, 105.0, 11.0, true, (gray_r, gray_g, gray_b), 0);
            let vol_str = format!("{}%", state.config.volume);
            draw_cocoa_text(&vol_str, 380.0, 105.0, 13.0, true, (text_r, text_g, text_b), 2);

            let vol_fill_x = 40.0 + (state.config.volume as f64 / 100.0) * 340.0;
            draw_line(40.0, 150.0, 380.0, 150.0, 6.0, (border_r, border_g, border_b));
            draw_line(40.0, 150.0, vol_fill_x, 150.0, 6.0, (accent_r, accent_g, accent_b));

            let thumb_rect = NSRect { origin: NSPoint { x: vol_fill_x - 9.0, y: (640.0 - 150.0) - 9.0 }, size: NSSize { width: 18.0, height: 18.0 } };
            let thumb: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:thumb_rect];
            let accent_c: id = msg_send![class!(NSColor), colorWithRed:accent_r green:accent_g blue:accent_b alpha:1.0];
            let () = msg_send![accent_c, setFill];
            let () = msg_send![thumb, fill];

            // 2. Interval Card
            draw_rounded_card(20.0, 195.0, 380.0, 90.0, 12.0, (card_r, card_g, card_b), (border_r, border_g, border_b));
            draw_cocoa_text("REMINDER INTERVAL", 40.0, 210.0, 11.0, true, (gray_r, gray_g, gray_b), 0);
            let int_str = format!("{} mins", state.config.interval_mins);
            draw_cocoa_text(&int_str, 380.0, 210.0, 13.0, true, (text_r, text_g, text_b), 2);

            let int_fill_x = 40.0 + (((state.config.interval_mins.clamp(5, 60) - 5) as f64) / 55.0) * 340.0;
            draw_line(40.0, 255.0, 380.0, 255.0, 6.0, (border_r, border_g, border_b));
            draw_line(40.0, 255.0, int_fill_x, 255.0, 6.0, (accent_r, accent_g, accent_b));

            let thumb_rect2 = NSRect { origin: NSPoint { x: int_fill_x - 9.0, y: (640.0 - 255.0) - 9.0 }, size: NSSize { width: 18.0, height: 18.0 } };
            let thumb2: id = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:thumb_rect2];
            let () = msg_send![accent_c, setFill];
            let () = msg_send![thumb2, fill];

            // 3. Auto-Start Card
            draw_rounded_card(20.0, 300.0, 380.0, 60.0, 12.0, (card_r, card_g, card_b), (border_r, border_g, border_b));
            draw_cocoa_text("Launch on macOS Startup", 40.0, 320.0, 13.0, true, (text_r, text_g, text_b), 0);
            draw_toggle(330.0, 315.0, state.config.run_at_startup);

            // 4. Test Sound Button
            draw_rounded_card(20.0, 375.0, 380.0, 44.0, 10.0, (accent_r, accent_g, accent_b), (accent_r, accent_g, accent_b));
            draw_cocoa_text("🔊 Test Zikr Sound", 210.0, 388.0, 13.0, true, (1.0, 1.0, 1.0), 1);

            // 5. Quit AutoZikr Button
            let (danger_r, danger_g, danger_b) = if is_dark { (255.0/255.0, 107.0/255.0, 107.0/255.0) } else { (220.0/255.0, 50.0/255.0, 50.0/255.0) };
            draw_rounded_card(20.0, 430.0, 380.0, 44.0, 10.0, (danger_r, danger_g, danger_b), (danger_r, danger_g, danger_b));
            draw_cocoa_text("Quit AutoZikr", 210.0, 443.0, 13.0, true, (1.0, 1.0, 1.0), 1);
        }

        // --- Bottom Navigation Tab Bar (Y: 570 to 640) ---
        let nav_y = 570.0;
        draw_line(0.0, nav_y, 420.0, nav_y, 1.0, (border_r, border_g, border_b));

        let tabs = ["Dashboard", "Settings"];
        for i in 0..2 {
            let active = state.current_tab == i as u32;
            let tab_x = (i as f64) * 210.0;
            if active {
                draw_line(tab_x + 30.0, nav_y, tab_x + 180.0, nav_y, 3.0, (accent_r, accent_g, accent_b));
            }
            let color = if active { (accent_r, accent_g, accent_b) } else { (gray_r, gray_g, gray_b) };
            draw_cocoa_text(tabs[i], tab_x + 105.0, nav_y + 24.0, 13.0, true, color, 1);
        }
    }
}

// Mouse event handlers
extern "C" fn view_mouse_down(this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let loc: NSPoint = msg_send![event, locationInWindow];
        let fx = loc.x as f64;
        let fy = (640.0 - loc.y) as f64; // Convert Cocoa bottom-left origin to top-left

        let state_arc = get_state();
        let mut state = state_arc.lock().unwrap();

        // 1. Navigation Tab Bar (Y: 570+)
        if fy >= 570.0 {
            let tab = (fx / 210.0) as u32;
            if tab < 2 && state.current_tab != tab {
                state.current_tab = tab;
                let () = msg_send![this, setNeedsDisplay:YES];
            }
            return;
        }

        // Header active toggle
        if fx >= 330.0 && fx <= 380.0 && fy >= 20.0 && fy <= 55.0 {
            state.config.enabled = !state.config.enabled;
            state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
            state.total_seconds = state.config.interval_mins * 60;
            state.config.save_to_file(&state.config_path);
            let () = msg_send![this, setNeedsDisplay:YES];
            return;
        }

        match state.current_tab {
            0 => {
                // Dashboard
                let scroll_y = state.dashboard_scroll as f64;
                let clicked_fy = fy + scroll_y;

                // Quiet hours master toggle
                if fx >= 330.0 && fx <= 380.0 && clicked_fy >= 295.0 && clicked_fy <= 335.0 {
                    state.config.quiet_hours_enabled = !state.config.quiet_hours_enabled;
                    state.config.save_to_file(&state.config_path);
                    let () = msg_send![this, setNeedsDisplay:YES];
                    return;
                }

                if state.config.quiet_hours_enabled {
                    let mut rule_y = 365.0;
                    for i in 0..state.quiet_hours_rules.len() {
                        // Rule toggle
                        if fx >= 50.0 && fx <= 98.0 && clicked_fy >= rule_y + 12.0 && clicked_fy <= rule_y + 38.0 {
                            state.quiet_hours_rules[i].enabled = !state.quiet_hours_rules[i].enabled;
                            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                            state.config.quiet_hours_rules = rules_str.join(";");
                            state.config.save_to_file(&state.config_path);
                            let () = msg_send![this, setNeedsDisplay:YES];
                            return;
                        }

                        // Delete rule
                        if fx >= 345.0 && fx <= 375.0 && clicked_fy >= rule_y + 10.0 && clicked_fy <= rule_y + 36.0 {
                            state.quiet_hours_rules.remove(i);
                            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                            state.config.quiet_hours_rules = rules_str.join(";");
                            state.config.save_to_file(&state.config_path);
                            let () = msg_send![this, setNeedsDisplay:YES];
                            return;
                        }

                        // Day bubbles
                        for d in 0..7 {
                            let cx_day = 55.0 + (d as f64 * 42.0);
                            let cy_day = rule_y + 82.0;
                            let dist = ((fx - cx_day).powi(2) + (clicked_fy - cy_day).powi(2)).sqrt();
                            if dist <= 14.0 {
                                state.quiet_hours_rules[i].days[d] = !state.quiet_hours_rules[i].days[d];
                                let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                                state.config.quiet_hours_rules = rules_str.join(";");
                                state.config.save_to_file(&state.config_path);
                                let () = msg_send![this, setNeedsDisplay:YES];
                                return;
                            }
                        }

                        // Sliders
                        if clicked_fy >= rule_y + 115.0 && clicked_fy <= rule_y + 140.0 && fx >= 40.0 && fx <= 360.0 {
                            let rule = &state.quiet_hours_rules[i];
                            let x_start = 50.0 + (rule.start_hour as f64 / 23.0) * 300.0;
                            let x_end = 50.0 + (rule.end_hour as f64 / 23.0) * 300.0;
                            state.active_rule_index = Some(i);
                            if (fx - x_start).abs() < (fx - x_end).abs() {
                                state.dragging_start_hour = true;
                            } else {
                                state.dragging_end_hour = true;
                            }
                            return;
                        }

                        // Overnight toggle
                        if fx >= 325.0 && fx <= 373.0 && clicked_fy >= rule_y + 145.0 && clicked_fy <= rule_y + 172.0 {
                            state.quiet_hours_rules[i].overnight = !state.quiet_hours_rules[i].overnight;
                            let r = &mut state.quiet_hours_rules[i];
                            std::mem::swap(&mut r.start_hour, &mut r.end_hour);
                            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                            state.config.quiet_hours_rules = rules_str.join(";");
                            state.config.save_to_file(&state.config_path);
                            let () = msg_send![this, setNeedsDisplay:YES];
                            return;
                        }

                        rule_y += 195.0;
                    }

                    // Add Rule button
                    if fx >= 35.0 && fx <= 385.0 && clicked_fy >= rule_y && clicked_fy <= rule_y + 36.0 {
                        state.quiet_hours_rules.push(QuietHoursRule {
                            enabled: true,
                            preset: "every_day".to_string(),
                            start_hour: 22,
                            end_hour: 8,
                            days: [true; 7],
                            overnight: true,
                        });
                        let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                        state.config.quiet_hours_rules = rules_str.join(";");
                        state.config.save_to_file(&state.config_path);
                        let () = msg_send![this, setNeedsDisplay:YES];
                        return;
                    }
                }
            }
            1 => {
                // Settings
                // Volume slider (Y: 135 to 165)
                if fy >= 135.0 && fy <= 165.0 && fx >= 30.0 && fx <= 390.0 {
                    state.volume_dragging = true;
                    let pct = ((fx - 40.0) / 340.0).clamp(0.0, 1.0);
                    state.config.volume = (pct * 20.0).round() as u32 * 5;
                    state.config.save_to_file(&state.config_path);
                    let () = msg_send![this, setNeedsDisplay:YES];
                }

                // Interval slider (Y: 240 to 270)
                if fy >= 240.0 && fy <= 270.0 && fx >= 30.0 && fx <= 390.0 {
                    state.interval_dragging = true;
                    let pct = ((fx - 40.0) / 340.0).clamp(0.0, 1.0);
                    let steps = (5.0 + (pct * 55.0)) / 5.0;
                    state.config.interval_mins = (steps.round() as u32 * 5).clamp(5, 60);
                    state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
                    state.total_seconds = state.config.interval_mins * 60;
                    state.config.save_to_file(&state.config_path);
                    let () = msg_send![this, setNeedsDisplay:YES];
                }

                // Auto-start toggle (Y: 300 to 360)
                if fx >= 330.0 && fx <= 380.0 && fy >= 305.0 && fy <= 345.0 {
                    state.config.run_at_startup = !state.config.run_at_startup;
                    let _ = set_run_at_startup(state.config.run_at_startup);
                    state.config.save_to_file(&state.config_path);
                    let () = msg_send![this, setNeedsDisplay:YES];
                }

                // Test sound button (Y: 375 to 420)
                if fx >= 20.0 && fx <= 400.0 && fy >= 375.0 && fy <= 420.0 {
                    let audio_files = crate::get_audio_files();
                    if !audio_files.is_empty() {
                        let rand_idx = crate::get_random_index(audio_files.len());
                        let selected = audio_files[rand_idx].clone();
                        let vol = state.config.volume;
                        if let Some(bytes) = crate::builtin_audio::get_builtin_bytes(&selected) {
                            play_sound_bytes(bytes, vol);
                        } else {
                            let audio_dir = crate::get_zikr_audio_dir();
                            let full_wav_path = audio_dir.join(&selected);
                            play_sound(&full_wav_path, vol);
                        }
                    }
                }

                // Quit AutoZikr button (Y: 430 to 475)
                if fx >= 20.0 && fx <= 400.0 && fy >= 430.0 && fy <= 475.0 {
                    std::process::exit(0);
                }
            }
            _ => {}
        }
    }
}

extern "C" fn view_mouse_dragged(this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let loc: NSPoint = msg_send![event, locationInWindow];
        let fx = loc.x as f64;
        let fy = (640.0 - loc.y) as f64;

        let state_arc = get_state();
        let mut state = state_arc.lock().unwrap();

        if state.current_tab == 1 {
            if state.volume_dragging {
                let pct = ((fx - 40.0) / 340.0).clamp(0.0, 1.0);
                state.config.volume = (pct * 20.0).round() as u32 * 5;
                let () = msg_send![this, setNeedsDisplay:YES];
            } else if state.interval_dragging {
                let pct = ((fx - 40.0) / 340.0).clamp(0.0, 1.0);
                let steps = (5.0 + (pct * 55.0)) / 5.0;
                state.config.interval_mins = (steps.round() as u32 * 5).clamp(5, 60);
                state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
                state.total_seconds = state.config.interval_mins * 60;
                let () = msg_send![this, setNeedsDisplay:YES];
            }
        } else if state.current_tab == 0 && state.active_rule_index.is_some() {
            let rule_idx = state.active_rule_index.unwrap();
            let pct = ((fx - 50.0) / 300.0).clamp(0.0, 1.0);
            let hour = (pct * 23.0).round() as u32;

            if state.dragging_start_hour {
                state.quiet_hours_rules[rule_idx].start_hour = hour;
            } else if state.dragging_end_hour {
                state.quiet_hours_rules[rule_idx].end_hour = hour;
            }
            state.quiet_hours_rules[rule_idx].overnight = state.quiet_hours_rules[rule_idx].start_hour > state.quiet_hours_rules[rule_idx].end_hour;
            let () = msg_send![this, setNeedsDisplay:YES];
        }
    }
}

extern "C" fn view_mouse_up(this: &Object, _cmd: Sel, _event: id) {
    unsafe {
        let state_arc = get_state();
        let mut state = state_arc.lock().unwrap();
        if state.volume_dragging || state.interval_dragging || state.active_rule_index.is_some() {
            state.volume_dragging = false;
            state.interval_dragging = false;
            state.active_rule_index = None;
            state.dragging_start_hour = false;
            state.dragging_end_hour = false;
            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
            state.config.quiet_hours_rules = rules_str.join(";");
            state.config.save_to_file(&state.config_path);
        }
        state.dashboard_dragging = false;
        state.scrollbar_dragging = false;
    }
}

extern "C" fn view_scroll_wheel(this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let delta_y: f64 = msg_send![event, scrollingDeltaY];
        let state_arc = get_state();
        let mut state = state_arc.lock().unwrap();
        if state.current_tab == 0 {
            let total_content_height = if state.config.quiet_hours_enabled {
                295.0 + 80.0 + (state.quiet_hours_rules.len() as f32 * 195.0) + 48.0
            } else {
                380.0
            };
            let max_scroll = (total_content_height - 480.0).max(0.0);
            state.dashboard_scroll = (state.dashboard_scroll - (delta_y as f32 * 0.8)).clamp(0.0, max_scroll);
            let () = msg_send![this, setNeedsDisplay:YES];
        }
    }
}

// --- Menu Bar Click Target ---

extern "C" fn status_bar_clicked(_this: &Object, _cmd: Sel) {
    unsafe {
        let event: id = msg_send![NSApp(), currentEvent];
        let event_type: NSUInteger = if event != nil { msg_send![event, type] } else { 0 };
        // NSEventTypeRightMouseUp = 3, NSEventTypeRightMouseDown = 2
        let is_right_click = event_type == 2 || event_type == 3;

        if is_right_click && GLOBAL_STATUS_ITEM != nil {
            let menu: id = msg_send![class!(NSMenu), alloc];
            let menu: id = msg_send![menu, init];

            let open_title = NSString::alloc(nil).init_str("Open Dashboard");
            let open_item: id = msg_send![class!(NSMenuItem), alloc];
            let open_item: id = msg_send![open_item, initWithTitle:open_title action:sel!(statusClicked:) keyEquivalent:NSString::alloc(nil).init_str("")];
            let () = msg_send![menu, addItem:open_item];

            let () = msg_send![menu, addItem:msg_send![class!(NSMenuItem), separatorItem]];

            let quit_title = NSString::alloc(nil).init_str("Quit AutoZikr");
            let quit_item: id = msg_send![class!(NSMenuItem), alloc];
            let quit_item: id = msg_send![quit_item, initWithTitle:quit_title action:sel!(terminate:) keyEquivalent:NSString::alloc(nil).init_str("q")];
            let () = msg_send![menu, addItem:quit_item];

            let () = msg_send![GLOBAL_STATUS_ITEM, popUpStatusItemMenu:menu];
            return;
        }

        if GLOBAL_POPOVER != nil && GLOBAL_STATUS_ITEM != nil {
            let is_shown: BOOL = msg_send![GLOBAL_POPOVER, isShown];
            if is_shown == YES {
                let () = msg_send![GLOBAL_POPOVER, performClose:nil];
            } else {
                let button: id = msg_send![GLOBAL_STATUS_ITEM, button];
                if button != nil {
                    let bounds: NSRect = msg_send![button, bounds];
                    // NSRectEdgeMinY = 1 (opens downwards from status bar)
                    let () = msg_send![GLOBAL_POPOVER, showRelativeToRect:bounds ofView:button preferredEdge:1 as NSUInteger];
                    if GLOBAL_VIEW != nil {
                        let () = msg_send![GLOBAL_VIEW, setNeedsDisplay:YES];
                    }
                }
            }
        }
    }
}

// --- Application Runner ---

pub fn run_macos_app() {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory); // Menu bar only

        // Load config
        let mut exe_dir = std::env::current_exe().unwrap_or_default();
        exe_dir.pop();
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

        if is_run_at_startup_enabled() {
            if !config.run_at_startup {
                config.run_at_startup = true;
                config.save_to_file(&config_path);
            }
        }

        if config.first_launch {
            config.first_launch = false;
            config.save_to_file(&config_path);
            let _ = Command::new("osascript")
                .arg("-e")
                .arg("display notification \"AutoZikr is running! Click the star icon in your Menu Bar.\" with title \"AutoZikr\"")
                .status();
        }

        let audio_files = crate::get_audio_files();
        let quiet_hours_rules = crate::config::parse_rules(&config.quiet_hours_rules);
        let remaining = crate::get_seconds_until_next_boundary(config.interval_mins);
        let total = config.interval_mins * 60;

        let state = Arc::new(Mutex::new(MacAppState {
            config,
            config_path,
            remaining_seconds: remaining,
            total_seconds: total,
            audio_files,
            current_tab: 0,
            volume_dragging: false,
            interval_dragging: false,
            is_dirty: false,
            quiet_hours_rules,
            active_rule_index: None,
            dragging_start_hour: false,
            dragging_end_hour: false,
            dashboard_scroll: 0.0,
            dashboard_dragging: false,
            scrollbar_dragging: false,
            drag_start_y: 0.0,
            drag_start_scroll: 0.0,
        }));
        GLOBAL_STATE = Some(Arc::clone(&state));

        // Register custom AutoZikrView class
        let superclass = class!(NSView);
        let mut decl = ClassDecl::new("AutoZikrView", superclass).expect("Failed to register AutoZikrView");
        decl.add_method(sel!(drawRect:), view_draw_rect as extern "C" fn(&Object, Sel, NSRect));
        decl.add_method(sel!(mouseDown:), view_mouse_down as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(mouseDragged:), view_mouse_dragged as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(mouseUp:), view_mouse_up as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(scrollWheel:), view_scroll_wheel as extern "C" fn(&Object, Sel, id));
        let view_class = decl.register();

        // Register Target action class for Status Item
        let mut target_decl = ClassDecl::new("AutoZikrStatusTarget", class!(NSObject)).expect("Failed to register AutoZikrStatusTarget");
        target_decl.add_method(sel!(statusClicked:), status_bar_clicked as extern "C" fn(&Object, Sel));
        let target_class = target_decl.register();
        let target_obj: id = msg_send![target_class, alloc];
        let target_obj: id = msg_send![target_obj, init];

        // Create Status Bar Item
        let status_bar = NSStatusBar::systemStatusBar(nil);
        let status_item = status_bar.statusItemWithLength_(-1.0); // NSSquareStatusItemLength
        GLOBAL_STATUS_ITEM = status_item;

        let status_button: id = msg_send![status_item, button];
        let star_icon = create_star_template_image(36.0);
        if star_icon != nil && status_button != nil {
            let () = msg_send![status_button, setImage:star_icon];
            let () = msg_send![status_button, setTarget:target_obj];
            let () = msg_send![status_button, setAction:sel!(statusClicked:)];
            // NSEventMaskLeftMouseUp (1<<1) | NSEventMaskRightMouseUp (1<<3)
            let () = msg_send![status_button, sendActionOn:( (1 << 1) | (1 << 3) ) as NSUInteger];
        }

        // Create Native NSPopover
        let popover: id = msg_send![class!(NSPopover), alloc];
        let popover: id = msg_send![popover, init];
        let () = msg_send![popover, setBehavior:NSPopoverBehaviorTransient];
        let () = msg_send![popover, setAnimates:YES];
        let () = msg_send![popover, setContentSize:NSSize { width: 420.0, height: 640.0 }];
        GLOBAL_POPOVER = popover;

        // Create Custom View inside View Controller
        let view_frame = NSRect { origin: NSPoint { x: 0.0, y: 0.0 }, size: NSSize { width: 420.0, height: 640.0 } };
        let custom_view: id = msg_send![view_class, alloc];
        let custom_view: id = msg_send![custom_view, initWithFrame:view_frame];
        GLOBAL_VIEW = custom_view;

        let view_controller: id = msg_send![class!(NSViewController), alloc];
        let view_controller: id = msg_send![view_controller, init];
        let () = msg_send![view_controller, setView:custom_view];
        let () = msg_send![popover, setContentViewController:view_controller];

        // Background Timer Thread
        let state_thread = Arc::clone(&state);
        std::thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                std::thread::sleep(Duration::from_secs(1));

                let now = Instant::now();
                let elapsed = now.duration_since(last_tick);
                last_tick = now;

                let mut state = state_thread.lock().unwrap();

                // Sleep detection: gap > 5s indicates system was asleep
                if elapsed > Duration::from_secs(5) {
                    state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
                    state.total_seconds = state.config.interval_mins * 60;
                    continue;
                }

                if state.config.enabled && !is_lid_closed() {
                    let in_quiet = crate::is_in_quiet_hours(&state.config);

                    if !in_quiet {
                        if state.remaining_seconds > 0 {
                            state.remaining_seconds -= 1;
                        } else {
                            state.audio_files = crate::get_audio_files();
                            if !state.audio_files.is_empty() && !is_audio_playing() {
                                let rand_idx = crate::get_random_index(state.audio_files.len());
                                let selected_file = state.audio_files[rand_idx].clone();
                                if let Some(bytes) = crate::builtin_audio::get_builtin_bytes(&selected_file) {
                                    play_sound_bytes(bytes, state.config.volume);
                                } else {
                                    let audio_dir = crate::get_zikr_audio_dir();
                                    let full_wav_path = audio_dir.join(&selected_file);
                                    play_sound(&full_wav_path, state.config.volume);
                                }
                            }
                            state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
                            state.total_seconds = state.config.interval_mins * 60;
                        }
                    }
                }

                // Request view redraw on main queue
                unsafe {
                    if GLOBAL_VIEW != nil {
                        let () = msg_send![GLOBAL_VIEW, performSelectorOnMainThread:sel!(setNeedsDisplay:) withObject:nil waitUntilDone:NO];
                    }
                }
            }
        });

        // Run Cocoa Native Event Loop
        app.run();
        let () = msg_send!(pool, drain);
    }
}
