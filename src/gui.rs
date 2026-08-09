use std::sync::{Arc, Mutex};
use std::path::PathBuf;

use windows::core::{w, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, SetWindowPos, HWND_TOPMOST, SWP_SHOWWINDOW,
    GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW, DefWindowProcW,
    WM_CREATE, WM_PAINT, WM_SIZE, WM_CLOSE, WM_DESTROY,
    WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_LBUTTONUP,
    IDC_ARROW, LoadCursorW, HCURSOR, IDI_APPLICATION, LoadIconW,
    ShowWindow, PostQuitMessage, SW_HIDE, SW_SHOW,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_RENDER_TARGET_PROPERTIES, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_ELLIPSE, D2D1_ARC_SEGMENT, D2D1_SWEEP_DIRECTION_CLOCKWISE,
    D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D_SIZE_U, D2D_POINT_2F, D2D1_COLOR_F,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_FACTORY_TYPE_SHARED,
};
use windows::Win32::Graphics::Gdi::{PAINTSTRUCT, BeginPaint, EndPaint, InvalidateRect};

use crate::config::{AppConfig, QuietHoursRule};

pub struct AppState {
    pub config: AppConfig,
    pub config_path: PathBuf,
    pub remaining_seconds: u32,
    pub total_seconds: u32,
    pub audio_files: Vec<String>, // WAV filenames in zikr_audio
    pub current_tab: u32, // 0: Dashboard, 1: Settings
    pub volume_dragging: bool,
    pub interval_dragging: bool,
    pub next_reminder_tick: bool, // trigger playing in background
    pub is_dirty: bool,

    // Quiet Hours rules editing state
    pub quiet_hours_rules: Vec<QuietHoursRule>,
    pub active_rule_index: Option<usize>, // rule currently being dragged
    pub dragging_start_hour: bool,
    pub dragging_end_hour: bool,

    // Dashboard scrolling state
    pub dashboard_scroll: i32,
    pub dashboard_dragging: bool,
    pub scrollbar_dragging: bool,
    pub drag_start_y: f32,
    pub drag_start_scroll: i32,
}

pub struct GuiContext {
    hwnd: HWND,
    state: Arc<Mutex<AppState>>,
    d2d_factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
    render_target: Option<ID2D1HwndRenderTarget>,
    
    // Brushes (Device-dependent)
    br_bg: Option<ID2D1SolidColorBrush>,
    br_card: Option<ID2D1SolidColorBrush>,
    br_accent: Option<ID2D1SolidColorBrush>,
    br_text_white: Option<ID2D1SolidColorBrush>,
    br_text_gray: Option<ID2D1SolidColorBrush>,
    br_border: Option<ID2D1SolidColorBrush>,
    br_danger: Option<ID2D1SolidColorBrush>,
    br_green: Option<ID2D1SolidColorBrush>,

    // Text Formats (Device-independent)
    fmt_title: IDWriteTextFormat,
    fmt_subtitle: IDWriteTextFormat,
    fmt_timer: IDWriteTextFormat,
    fmt_body: IDWriteTextFormat,
    fmt_body_bold: IDWriteTextFormat,
    pub last_hide_time: std::cell::Cell<std::time::Instant>,
    theme_is_dark: bool,
}

impl GuiContext {
    pub fn new(hwnd: HWND, state: Arc<Mutex<AppState>>) -> Self {
        let d2d_factory = unsafe {
            windows::Win32::Graphics::Direct2D::D2D1CreateFactory(
                windows::Win32::Graphics::Direct2D::D2D1_FACTORY_TYPE_SINGLE_THREADED,
                None,
            )
            .unwrap()
        };

        let dwrite_factory: IDWriteFactory = unsafe {
            windows::Win32::Graphics::DirectWrite::DWriteCreateFactory(
                windows::Win32::Graphics::DirectWrite::DWRITE_FACTORY_TYPE_SHARED,
            )
            .unwrap()
        };

        // Initialize DirectWrite Text Formats
        let fmt_title = unsafe {
            dwrite_factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    20.0,
                    w!("en-us"),
                )
                .unwrap()
        };

        let fmt_subtitle = unsafe {
            dwrite_factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_REGULAR,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    12.0,
                    w!("en-us"),
                )
                .unwrap()
        };

        let fmt_timer = unsafe {
            dwrite_factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    36.0,
                    w!("en-us"),
                )
                .unwrap()
        };

        let fmt_body = unsafe {
            dwrite_factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_REGULAR,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    14.0,
                    w!("en-us"),
                )
                .unwrap()
        };

        let fmt_body_bold = unsafe {
            dwrite_factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    14.0,
                    w!("en-us"),
                )
                .unwrap()
        };

        unsafe {
            let _ = fmt_title.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            let _ = fmt_title.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            let _ = fmt_subtitle.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = fmt_timer.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            let _ = fmt_timer.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
        }

        Self {
            hwnd,
            state,
            d2d_factory,
            dwrite_factory,
            render_target: None,
            br_bg: None,
            br_card: None,
            br_accent: None,
            br_text_white: None,
            br_text_gray: None,
            br_border: None,
            br_danger: None,
            br_green: None,
            fmt_title,
            fmt_subtitle,
            fmt_timer,
            fmt_body,
            fmt_body_bold,
            last_hide_time: std::cell::Cell::new(std::time::Instant::now() - std::time::Duration::from_secs(10)),
            theme_is_dark: is_dark_mode(),
        }
    }

    fn draw_text(
        &self,
        rt: &ID2D1HwndRenderTarget,
        text: &str,
        format: &IDWriteTextFormat,
        rect: &D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        let text_utf16: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let _ = rt.DrawText(
                &text_utf16,
                format,
                rect,
                brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    fn ensure_resources(&mut self) -> Result<()> {
        let current_dark = is_dark_mode();
        if self.render_target.is_some() && self.theme_is_dark != current_dark {
            self.discard_resources();
        }
        self.theme_is_dark = current_dark;

        if self.render_target.is_some() {
            return Ok(());
        }

        let mut rect = RECT::default();
        unsafe { GetClientRect(self.hwnd, &mut rect)? };
        
        let size = D2D_SIZE_U {
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
        };

        unsafe {
            let rt_props = D2D1_RENDER_TARGET_PROPERTIES::default();
            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: self.hwnd,
                pixelSize: size,
                presentOptions: windows::Win32::Graphics::Direct2D::D2D1_PRESENT_OPTIONS_NONE,
            };

            let rt = self.d2d_factory.CreateHwndRenderTarget(&rt_props, &hwnd_props)?;

            let (bg_c, card_c, accent_c, text_p, text_s, border_c, danger_c, green_c) = if current_dark {
                (
                    D2D1_COLOR_F { r: 15.0 / 255.0, g: 15.0 / 255.0, b: 19.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 27.0 / 255.0, g: 27.0 / 255.0, b: 34.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 177.0 / 255.0, g: 159.0 / 255.0, b: 251.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
                    D2D1_COLOR_F { r: 142.0 / 255.0, g: 140.0 / 255.0, b: 154.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 44.0 / 255.0, g: 44.0 / 255.0, b: 53.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 255.0 / 255.0, g: 107.0 / 255.0, b: 107.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 76.0 / 255.0, g: 175.0 / 255.0, b: 80.0 / 255.0, a: 1.0 },
                )
            } else {
                (
                    D2D1_COLOR_F { r: 240.0 / 255.0, g: 240.0 / 255.0, b: 245.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 255.0 / 255.0, g: 255.0 / 255.0, b: 255.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 120.0 / 255.0, g: 90.0 / 255.0, b: 230.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 30.0 / 255.0, g: 30.0 / 255.0, b: 40.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 110.0 / 255.0, g: 110.0 / 255.0, b: 125.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 215.0 / 255.0, g: 215.0 / 255.0, b: 225.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 220.0 / 255.0, g: 50.0 / 255.0, b: 50.0 / 255.0, a: 1.0 },
                    D2D1_COLOR_F { r: 40.0 / 255.0, g: 160.0 / 255.0, b: 90.0 / 255.0, a: 1.0 },
                )
            };

            self.br_bg = Some(rt.CreateSolidColorBrush(&bg_c, None)?);
            self.br_card = Some(rt.CreateSolidColorBrush(&card_c, None)?);
            self.br_accent = Some(rt.CreateSolidColorBrush(&accent_c, None)?);
            self.br_text_white = Some(rt.CreateSolidColorBrush(&text_p, None)?);
            self.br_text_gray = Some(rt.CreateSolidColorBrush(&text_s, None)?);
            self.br_border = Some(rt.CreateSolidColorBrush(&border_c, None)?);
            self.br_danger = Some(rt.CreateSolidColorBrush(&danger_c, None)?);
            self.br_green = Some(rt.CreateSolidColorBrush(&green_c, None)?);

            self.render_target = Some(rt);
        }

        Ok(())
    }

    fn discard_resources(&mut self) {
        self.render_target = None;
        self.br_bg = None;
        self.br_card = None;
        self.br_accent = None;
        self.br_text_white = None;
        self.br_text_gray = None;
        self.br_border = None;
        self.br_danger = None;
        self.br_green = None;
    }

    fn position_window_at_tray(&self) {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetCursorPos, SystemParametersInfoW, SetWindowPos, SPI_GETWORKAREA, HWND_TOPMOST,
                SWP_SHOWWINDOW
            };
            use windows::Win32::Foundation::POINT;
            let mut cursor_pos = POINT::default();
            let _ = GetCursorPos(&mut cursor_pos);

            let mut work_area = RECT::default();
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut work_area as *mut RECT as *mut std::ffi::c_void),
                windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );

            let win_width = 420;
            let win_height = 620;

            // Center window horizontally on cursor, clamp within monitor work area
            let mut x = cursor_pos.x - (win_width / 2);
            if x < work_area.left {
                x = work_area.left;
            } else if x + win_width > work_area.right {
                x = work_area.right - win_width;
            }

            // Position vertically just above the bottom taskbar or below top taskbar
            let mut y = work_area.bottom - win_height;
            if cursor_pos.y < (work_area.top + work_area.bottom) / 2 {
                y = work_area.top;
            }

            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                win_width,
                win_height,
                SWP_SHOWWINDOW,
            );
        }
    }

    fn handle_tray_icon(&self, lparam: LPARAM) {
        let event = lparam.0 as u32;
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                WM_LBUTTONUP, WM_RBUTTONUP, IsWindowVisible, ShowWindow, SW_HIDE, SetForegroundWindow
            };
            match event {
                WM_LBUTTONUP => {
                    let is_visible = IsWindowVisible(self.hwnd).as_bool();
                    let just_hidden = self.last_hide_time.get().elapsed().as_millis() < 250;
                    if is_visible {
                        let _ = ShowWindow(self.hwnd, SW_HIDE);
                        self.last_hide_time.set(std::time::Instant::now());
                    } else if just_hidden {
                        // Window was just hidden by deactivation from this click; keep it hidden.
                    } else {
                        self.position_window_at_tray();
                        let _ = SetForegroundWindow(self.hwnd);
                        unsafe { windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(self.hwnd); }
                    }
                }
                WM_RBUTTONUP => {
                    let enabled = self.state.lock().unwrap().config.enabled;
                    crate::tray::show_context_menu(self.hwnd, enabled);
                }
                _ => {}
            }
        }
    }

    fn handle_command(&self, wparam: WPARAM) {
        let cmd_id = (wparam.0 & 0xFFFF) as u32;
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                ShowWindow, SW_SHOW, SetForegroundWindow, DestroyWindow
            };
            use windows::Win32::Graphics::Gdi::InvalidateRect;
            use crate::tray::{ID_TRAY_OPEN, ID_TRAY_TOGGLE, ID_TRAY_EXIT, update_tray_status, remove_tray_icon};
            match cmd_id {
                ID_TRAY_OPEN => {
                    self.position_window_at_tray();
                    let _ = SetForegroundWindow(self.hwnd);
                    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(self.hwnd); }
                }
                ID_TRAY_TOGGLE => {
                    let mut s = self.state.lock().unwrap();
                    s.config.enabled = !s.config.enabled;
                    s.remaining_seconds = s.config.interval_mins * 60;
                    s.total_seconds = s.remaining_seconds;
                    s.config.save_to_file(&s.config_path);
                    let _ = update_tray_status(self.hwnd, s.config.enabled);
                    let _ = InvalidateRect(self.hwnd, None, true);
                }
                ID_TRAY_EXIT => {
                    let _ = remove_tray_icon(self.hwnd);
                    let _ = DestroyWindow(self.hwnd);
                }
                _ => {}
            }
        }
    }

    pub fn paint(&mut self) {
        if self.ensure_resources().is_err() {
            return;
        }

        let rt = self.render_target.as_ref().unwrap();

        unsafe {
            rt.BeginDraw();
            let bg_color = if self.theme_is_dark {
                D2D1_COLOR_F { r: 15.0 / 255.0, g: 15.0 / 255.0, b: 19.0 / 255.0, a: 1.0 }
            } else {
                D2D1_COLOR_F { r: 240.0 / 255.0, g: 240.0 / 255.0, b: 245.0 / 255.0, a: 1.0 }
            };
            rt.Clear(Some(&bg_color));

            let state = self.state.lock().unwrap();
            
            // Draw title header
            self.draw_header(rt, &state);

            // Draw current active tab view
            match state.current_tab {
                0 => self.draw_dashboard(rt, &state),
                1 => self.draw_settings(rt, &state),
                _ => {}
            }

            // Draw bottom navigation tab bar
            self.draw_navigation(rt, &state);

            std::mem::drop(state);

            let res = rt.EndDraw(None, None);
            if res.is_err() {
                self.discard_resources();
            }
        }
    }

    fn draw_header(&self, rt: &ID2D1HwndRenderTarget, state: &AppState) {
        let text_white = self.br_text_white.as_ref().unwrap();
        let text_gray = self.br_text_gray.as_ref().unwrap();

        let rect_t = D2D_RECT_F {
            left: 20.0,
            top: 25.0,
            right: 250.0,
            bottom: 55.0,
        };
        let rect_s = D2D_RECT_F {
            left: 20.0,
            top: 55.0,
            right: 250.0,
            bottom: 75.0,
        };

        unsafe {
            let _ = self.fmt_title.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
            let _ = self.fmt_title.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
            let _ = self.fmt_subtitle.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
        }

        self.draw_text(rt, "AutoZikr", &self.fmt_title, &rect_t, text_white);
        self.draw_text(rt, "Remember to Remember", &self.fmt_subtitle, &rect_s, text_gray);

        // Draw Active label and toggle switch on the right
        let rect_active = D2D_RECT_F {
            left: 200.0,
            top: 33.0,
            right: 320.0,
            bottom: 59.0,
        };
        unsafe {
            let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING);
            let _ = self.fmt_subtitle.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
        }
        let active_brush = if state.config.enabled { text_white } else { text_gray };
        self.draw_text(rt, "Active", &self.fmt_subtitle, &rect_active, active_brush);

        self.draw_toggle_switch(rt, 330.0, 33.0, state.config.enabled);
    }

    fn draw_dashboard(&self, rt: &ID2D1HwndRenderTarget, state: &AppState) {
        let card_brush = self.br_card.as_ref().unwrap();
        let border_brush = self.br_border.as_ref().unwrap();
        let text_white = self.br_text_white.as_ref().unwrap();
        let text_gray = self.br_text_gray.as_ref().unwrap();
        let accent_brush = self.br_accent.as_ref().unwrap();

        let scroll_y = state.dashboard_scroll as f32;
        let viewport_height = 475.0; // from 85 to 560

        // Push clip region for scrollable viewport
        unsafe {
            rt.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: 0.0,
                    top: 85.0,
                    right: 420.0,
                    bottom: 560.0,
                },
                windows::Win32::Graphics::Direct2D::D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            );
        }

        // 1. Draw Countdown timer card at top (Y: 90 to 290 offset by scroll)
        let card_timer_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 20.0,
                top: 90.0 - scroll_y,
                right: 400.0,
                bottom: 290.0 - scroll_y,
            },
            radiusX: 12.0,
            radiusY: 12.0,
        };

        unsafe {
            rt.FillRoundedRectangle(&card_timer_rect, card_brush);
            rt.DrawRoundedRectangle(&card_timer_rect, border_brush, 1.0, None);
        }

        // Draw progress ring centered inside top card
        let cx = 210.0;
        let cy = 190.0 - scroll_y;
        let r = 70.0;
        let stroke = 10.0;

        if state.config.enabled {
            unsafe {
                rt.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: cx, y: cy },
                        radiusX: r,
                        radiusY: r,
                    },
                    border_brush,
                    stroke,
                    None,
                );
            }

            let in_quiet = crate::is_in_quiet_hours(&state.config);
            let ring_brush = if in_quiet { border_brush } else { accent_brush };
            let label_str = if in_quiet { "QUIET HOURS" } else { "NEXT REMINDER" };

            let percent = if state.total_seconds > 0 {
                (state.remaining_seconds as f32 / state.total_seconds as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let _ = self.draw_progress_ring(rt, cx, cy, r, percent, stroke, ring_brush);

            let mins = state.remaining_seconds / 60;
            let secs = state.remaining_seconds % 60;
            let time_str = format!("{:02}:{:02}", mins, secs);

            let rect_timer = D2D_RECT_F {
                left: cx - 80.0,
                top: cy - 25.0,
                right: cx + 80.0,
                bottom: cy + 10.0,
            };
            let rect_timer_label = D2D_RECT_F {
                left: cx - 80.0,
                top: cy + 15.0,
                right: cx + 80.0,
                bottom: cy + 30.0,
            };

            unsafe {
                let _ = self.fmt_timer.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                let _ = self.fmt_timer.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                let _ = self.fmt_subtitle.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            }

            self.draw_text(rt, &time_str, &self.fmt_timer, &rect_timer, text_white);
            self.draw_text(rt, label_str, &self.fmt_subtitle, &rect_timer_label, text_gray);
        } else {
            let rect_t = D2D_RECT_F {
                left: cx - 150.0,
                top: cy - 20.0,
                right: cx + 150.0,
                bottom: cy + 5.0,
            };
            let rect_d = D2D_RECT_F {
                left: cx - 150.0,
                top: cy + 12.0,
                right: cx + 150.0,
                bottom: cy + 45.0,
            };

            unsafe {
                let _ = self.fmt_body_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                let _ = self.fmt_body_bold.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                let _ = self.fmt_subtitle.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            }

            self.draw_text(rt, "Reminders are Off", &self.fmt_body_bold, &rect_t, text_white);
            self.draw_text(rt, "Toggle 'Active' at top-right to start timer.", &self.fmt_subtitle, &rect_d, text_gray);
        }

        // 2. Draw Quiet Hours rules card below (Y: 300 to dynamic bottom offset by scroll)
        let rule_height = 185.0;
        let spacing = 15.0;
        let inner_y_start = 300.0 + 70.0;
        let add_btn_y = inner_y_start + (state.quiet_hours_rules.len() as f32 * (rule_height + spacing));
        let card_qh_bottom = if state.config.quiet_hours_enabled {
            add_btn_y + 40.0 + 15.0
        } else {
            300.0 + 130.0
        };

        let card_qh_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 20.0,
                top: 300.0 - scroll_y,
                right: 400.0,
                bottom: card_qh_bottom - scroll_y,
            },
            radiusX: 12.0,
            radiusY: 12.0,
        };

        unsafe {
            rt.FillRoundedRectangle(&card_qh_rect, card_brush);
            rt.DrawRoundedRectangle(&card_qh_rect, border_brush, 1.0, None);

            // Title & Description
            let rect_title = D2D_RECT_F {
                left: 40.0,
                top: 315.0 - scroll_y,
                right: 320.0,
                bottom: 335.0 - scroll_y,
            };
            let rect_desc = D2D_RECT_F {
                left: 40.0,
                top: 338.0 - scroll_y,
                right: 320.0,
                bottom: 358.0 - scroll_y,
            };

            let _ = self.fmt_body_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
            self.draw_text(rt, "Quiet Hours", &self.fmt_body_bold, &rect_title, text_white);
            
            let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
            self.draw_text(rt, "Silence reminders during custom times", &self.fmt_subtitle, &rect_desc, text_gray);

            // Master Toggle Switch
            self.draw_toggle_switch(rt, 330.0, 315.0 - scroll_y, state.config.quiet_hours_enabled);
        }

        // Draw rule sub-cards nestled inside
        if state.config.quiet_hours_enabled {
            for (i, rule) in state.quiet_hours_rules.iter().enumerate() {
                let y_offset = inner_y_start + (i as f32 * (rule_height + spacing)) - scroll_y;
                let r_rule = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: 35.0,
                        top: y_offset,
                        right: 385.0,
                        bottom: y_offset + rule_height,
                    },
                    radiusX: 8.0,
                    radiusY: 8.0,
                };

                unsafe {
                    // Fill nestled rules cards with window background color
                    rt.FillRoundedRectangle(&r_rule, self.br_bg.as_ref().unwrap());
                    rt.DrawRoundedRectangle(&r_rule, border_brush, 1.0, None);

                    // Rule Enabled toggle
                    self.draw_small_toggle_switch(rt, 50.0, y_offset + 15.0, rule.enabled);

                    // Preset Dropdown Button
                    let r_dropdown = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: 105.0,
                            top: y_offset + 13.0,
                            right: 215.0,
                            bottom: y_offset + 39.0,
                        },
                        radiusX: 6.0,
                        radiusY: 6.0,
                    };
                    rt.FillRoundedRectangle(&r_dropdown, card_brush);
                    rt.DrawRoundedRectangle(&r_dropdown, border_brush, 1.0, None);

                    let display_preset = match rule.preset.as_str() {
                        "every_day" => "Every Day ▼",
                        "work_days" => "Work Days ▼",
                        "weekends" => "Weekends ▼",
                        _ => "Custom ▼",
                    };
                    let rect_drop_txt = D2D_RECT_F {
                        left: 105.0,
                        top: y_offset + 13.0,
                        right: 215.0,
                        bottom: y_offset + 39.0,
                    };
                    let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                    let _ = self.fmt_subtitle.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                    self.draw_text(rt, display_preset, &self.fmt_subtitle, &rect_drop_txt, text_white);

                    // Draw Delete (trash can) button
                    let t_x = 355.0;
                    let t_y = y_offset + 15.0;
                    rt.DrawRectangle(
                        &D2D_RECT_F {
                            left: t_x,
                            top: t_y + 3.0,
                            right: t_x + 10.0,
                            bottom: t_y + 12.0,
                        },
                        accent_brush, // subtle delete
                        1.5,
                        None,
                    );
                    rt.DrawLine(D2D_POINT_2F { x: t_x - 3.0, y: t_y + 2.0 }, D2D_POINT_2F { x: t_x + 13.0, y: t_y + 2.0 }, accent_brush, 1.5, None);
                    rt.DrawLine(D2D_POINT_2F { x: t_x + 2.0, y: t_y }, D2D_POINT_2F { x: t_x + 8.0, y: t_y }, accent_brush, 1.5, None);

                    // Mute period summary text
                    let val_str = format!("Mute Period: {:02}:00 - {:02}:00{}", rule.start_hour, rule.end_hour, if rule.overnight { " (Overnight)" } else { "" });
                    let rect_val = D2D_RECT_F {
                        left: 50.0,
                        top: y_offset + 48.0,
                        right: 370.0,
                        bottom: y_offset + 68.0,
                    };
                    let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
                    self.draw_text(rt, &val_str, &self.fmt_subtitle, &rect_val, accent_brush);

                    // Day Bubbles
                    let day_labels = ["M", "T", "W", "T", "F", "S", "S"];
                    for d in 0..7 {
                        let cx_day = 55.0 + d as f32 * 42.0;
                        let cy_day = y_offset + 92.0;
                        
                        let is_active = rule.days[d];
                        let bubble_brush = if is_active { accent_brush } else { border_brush };
                        let label_brush = if is_active {
                            if self.theme_is_dark { card_brush } else { text_white }
                        } else {
                            text_gray
                        };

                        rt.FillEllipse(
                            &D2D1_ELLIPSE {
                                point: D2D_POINT_2F { x: cx_day, y: cy_day },
                                radiusX: 14.0,
                                radiusY: 14.0,
                            },
                            bubble_brush,
                        );

                        let rect_lbl = D2D_RECT_F {
                            left: cx_day - 14.0,
                            top: cy_day - 14.0,
                            right: cx_day + 14.0,
                            bottom: cy_day + 14.0,
                        };
                        let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                        let _ = self.fmt_subtitle.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                        self.draw_text(rt, day_labels[d], &self.fmt_subtitle, &rect_lbl, label_brush);
                    }

                    // Double Range Slider
                    let track_y = y_offset + 135.0;
                    let start_x = 50.0;
                    let end_x = 350.0;
                    let width = end_x - start_x;
                    
                    let x_start = start_x + (rule.start_hour as f32 / 23.0) * width;
                    let x_end = start_x + (rule.end_hour as f32 / 23.0) * width;

                    // Track background line
                    rt.DrawLine(
                        D2D_POINT_2F { x: start_x, y: track_y },
                        D2D_POINT_2F { x: end_x, y: track_y },
                        border_brush,
                        6.0,
                        None,
                    );

                    // Track active range highlights
                    if !rule.overnight {
                        // Standard range
                        rt.DrawLine(
                            D2D_POINT_2F { x: x_start, y: track_y },
                            D2D_POINT_2F { x: x_end, y: track_y },
                            accent_brush,
                            6.0,
                            None,
                        );
                    } else {
                        // Spans midnight: fill outer ends
                        rt.DrawLine(
                            D2D_POINT_2F { x: start_x, y: track_y },
                            D2D_POINT_2F { x: x_end, y: track_y },
                            accent_brush,
                            6.0,
                            None,
                        );
                        rt.DrawLine(
                            D2D_POINT_2F { x: x_start, y: track_y },
                            D2D_POINT_2F { x: end_x, y: track_y },
                            accent_brush,
                            6.0,
                            None,
                        );
                    }

                    // Draw vertical pill white thumbs
                    let thumb_width = 6.0;
                    let thumb_height = 20.0;
                    
                    let r_start_thumb = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: x_start - (thumb_width / 2.0),
                            top: track_y - (thumb_height / 2.0),
                            right: x_start + (thumb_width / 2.0),
                            bottom: track_y + (thumb_height / 2.0),
                        },
                        radiusX: 3.0,
                        radiusY: 3.0,
                    };
                    rt.FillRoundedRectangle(&r_start_thumb, text_white);
                    rt.DrawRoundedRectangle(&r_start_thumb, border_brush, 1.0, None);

                    let r_end_thumb = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: x_end - (thumb_width / 2.0),
                            top: track_y - (thumb_height / 2.0),
                            right: x_end + (thumb_width / 2.0),
                            bottom: track_y + (thumb_height / 2.0),
                        },
                        radiusX: 3.0,
                        radiusY: 3.0,
                    };
                    rt.FillRoundedRectangle(&r_end_thumb, text_white);
                    rt.DrawRoundedRectangle(&r_end_thumb, border_brush, 1.0, None);

                    // Overnight Spans Midnight toggle row
                    let rect_on = D2D_RECT_F {
                        left: 50.0,
                        top: y_offset + 158.0,
                        right: 280.0,
                        bottom: y_offset + 178.0,
                    };
                    let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
                    self.draw_text(rt, "Overnight (Spans Midnight)", &self.fmt_subtitle, &rect_on, text_gray);
                    
                    self.draw_small_toggle_switch(rt, 320.0, y_offset + 158.0, rule.overnight);
                }
            }

            // Draw Add Rule Card/Button nestled at bottom
            let add_y = add_btn_y - scroll_y;
            let r_add_btn = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 35.0,
                    top: add_y,
                    right: 385.0,
                    bottom: add_y + 40.0,
                },
                radiusX: 8.0,
                radiusY: 8.0,
            };
            unsafe {
                rt.FillRoundedRectangle(&r_add_btn, self.br_bg.as_ref().unwrap());
                rt.DrawRoundedRectangle(&r_add_btn, border_brush, 1.0, None);
                
                let rect_add_txt = D2D_RECT_F {
                    left: 35.0,
                    top: add_y,
                    right: 385.0,
                    bottom: add_y + 40.0,
                };
                let _ = self.fmt_body_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                let _ = self.fmt_body_bold.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                self.draw_text(rt, "+ Add Rule", &self.fmt_body_bold, &rect_add_txt, accent_brush);
            }
        } else {
            let info_y = inner_y_start - scroll_y;
            let rect_info = D2D_RECT_F {
                left: 40.0,
                top: info_y,
                right: 380.0,
                bottom: info_y + 50.0,
            };
            let _ = unsafe { self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING) };
            self.draw_text(rt, "Mute rules are suspended. Enable to schedule custom mute ranges.", &self.fmt_subtitle, &rect_info, text_gray);
        }

        unsafe {
            rt.PopAxisAlignedClip();
        }

        // Draw visual scrollbar for Dashboard
        let total_content_height = card_qh_bottom + 20.0;

        if total_content_height > viewport_height {
            let scroll_max = total_content_height - viewport_height;
            let scroll_pct = (state.dashboard_scroll as f32 / scroll_max).clamp(0.0, 1.0);
            
            let track_top = 100.0;
            let track_bottom = 540.0;
            let track_height = track_bottom - track_top;
            
            let thumb_height = (track_height * (viewport_height / total_content_height)).max(30.0);
            let thumb_top = track_top + scroll_pct * (track_height - thumb_height);
            
            let r_thumb = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 408.0,
                    top: thumb_top,
                    right: 414.0,
                    bottom: thumb_top + thumb_height,
                },
                radiusX: 3.0,
                radiusY: 3.0,
            };
            
            let r_track = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 409.0,
                    top: track_top,
                    right: 413.0,
                    bottom: track_bottom,
                },
                radiusX: 2.0,
                radiusY: 2.0,
            };
            
            unsafe {
                rt.FillRoundedRectangle(&r_track, border_brush);
                rt.FillRoundedRectangle(&r_thumb, accent_brush);
            }
        }
    }

    fn draw_settings(&self, rt: &ID2D1HwndRenderTarget, state: &AppState) {
        let card_brush = self.br_card.as_ref().unwrap();
        let border_brush = self.br_border.as_ref().unwrap();
        let text_white = self.br_text_white.as_ref().unwrap();
        let text_gray = self.br_text_gray.as_ref().unwrap();
        let accent_brush = self.br_accent.as_ref().unwrap();

        // 1. Volume Card
        let r_volume = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 20.0,
                top: 90.0,
                right: 400.0,
                bottom: 180.0,
            },
            radiusX: 12.0,
            radiusY: 12.0,
        };
        unsafe {
            rt.FillRoundedRectangle(&r_volume, card_brush);
            rt.DrawRoundedRectangle(&r_volume, border_brush, 1.0, None);

            // Volume Label & Value
            let rect_vt = D2D_RECT_F {
                left: 40.0,
                top: 105.0,
                right: 250.0,
                bottom: 125.0,
            };
            let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
            self.draw_text(rt, "REMINDER VOLUME", &self.fmt_subtitle, &rect_vt, text_gray);

            let vol_val_str = format!("{}%", state.config.volume);
            let rect_vv = D2D_RECT_F {
                left: 250.0,
                top: 105.0,
                right: 380.0,
                bottom: 125.0,
            };
            let _ = self.fmt_body_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING);
            self.draw_text(rt, &vol_val_str, &self.fmt_body_bold, &rect_vv, text_white);

            // Draw volume slider bar
            let slider_y = 145.0;
            let start_x = 40.0;
            let end_x = 380.0;
            let width = end_x - start_x;
            let fill_x = start_x + (state.config.volume as f32 / 100.0) * width;

            // Gray background line
            rt.DrawLine(
                D2D_POINT_2F { x: start_x, y: slider_y },
                D2D_POINT_2F { x: end_x, y: slider_y },
                border_brush,
                6.0,
                None,
            );
            // Filled line
            rt.DrawLine(
                D2D_POINT_2F { x: start_x, y: slider_y },
                D2D_POINT_2F { x: fill_x, y: slider_y },
                accent_brush,
                6.0,
                None,
            );
            // Slider Thumb
            rt.FillEllipse(
                &D2D1_ELLIPSE {
                    point: D2D_POINT_2F { x: fill_x, y: slider_y },
                    radiusX: 10.0,
                    radiusY: 10.0,
                },
                accent_brush,
            );
        }

        // 2. Interval Card (Now a Premium Slider, 90px tall)
        let r_interval = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 20.0,
                top: 195.0,
                right: 400.0,
                bottom: 285.0,
            },
            radiusX: 12.0,
            radiusY: 12.0,
        };
        unsafe {
            rt.FillRoundedRectangle(&r_interval, card_brush);
            rt.DrawRoundedRectangle(&r_interval, border_brush, 1.0, None);

            let rect_it = D2D_RECT_F {
                left: 40.0,
                top: 210.0,
                right: 250.0,
                bottom: 230.0,
            };
            let _ = self.fmt_subtitle.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
            self.draw_text(rt, "REMINDER INTERVAL", &self.fmt_subtitle, &rect_it, text_gray);

            let int_val_str = format!("{} mins", state.config.interval_mins);
            let rect_iv = D2D_RECT_F {
                left: 250.0,
                top: 210.0,
                right: 380.0,
                bottom: 230.0,
            };
            let _ = self.fmt_body_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING);
            self.draw_text(rt, &int_val_str, &self.fmt_body_bold, &rect_iv, text_white);

            // Draw interval slider bar (1 to 60 mins range)
            let slider_y = 250.0;
            let start_x = 40.0;
            let end_x = 380.0;
            let width = end_x - start_x;
            let fill_x = start_x + (((state.config.interval_mins.clamp(1, 60) - 1) as f32) / 59.0) * width;

            // Background line
            rt.DrawLine(
                D2D_POINT_2F { x: start_x, y: slider_y },
                D2D_POINT_2F { x: end_x, y: slider_y },
                border_brush,
                6.0,
                None,
            );
            // Filled line
            rt.DrawLine(
                D2D_POINT_2F { x: start_x, y: slider_y },
                D2D_POINT_2F { x: fill_x, y: slider_y },
                accent_brush,
                6.0,
                None,
            );
            // Slider Thumb
            rt.FillEllipse(
                &D2D1_ELLIPSE {
                    point: D2D_POINT_2F { x: fill_x, y: slider_y },
                    radiusX: 10.0,
                    radiusY: 10.0,
                },
                accent_brush,
            );
        }

        // 3. Auto-Start Card (Positioned compactly at top 300px)
        let r_startup = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 20.0,
                top: 300.0,
                right: 400.0,
                bottom: 360.0,
            },
            radiusX: 12.0,
            radiusY: 12.0,
        };
        unsafe {
            rt.FillRoundedRectangle(&r_startup, card_brush);
            rt.DrawRoundedRectangle(&r_startup, border_brush, 1.0, None);

            let rect_sl = D2D_RECT_F {
                left: 40.0,
                top: 300.0,
                right: 320.0,
                bottom: 360.0,
            };
            let _ = self.fmt_body_bold.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = self.fmt_body_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
            let startup_label = if cfg!(target_os = "macos") {
                "Launch on macOS Startup"
            } else {
                "Launch on Windows Startup"
            };
            self.draw_text(rt, startup_label, &self.fmt_body_bold, &rect_sl, text_white);

            self.draw_toggle_switch(rt, 330.0, 315.0, state.config.run_at_startup);
        }
    }

    fn draw_navigation(&self, rt: &ID2D1HwndRenderTarget, state: &AppState) {
        let border_brush = self.br_border.as_ref().unwrap();
        let text_gray = self.br_text_gray.as_ref().unwrap();
        let accent_brush = self.br_accent.as_ref().unwrap();

        let nav_y = 560.0;
        
        // Draw top line border of the navigation
        unsafe {
            rt.DrawLine(
                D2D_POINT_2F { x: 0.0, y: nav_y },
                D2D_POINT_2F { x: 420.0, y: nav_y },
                border_brush,
                1.5,
                None,
            );
        }

        let tabs = [
            "Dashboard",
            "Settings",
        ];

        let col_width = 420.0 / 2.0;
        for i in 0..2 {
            let active = state.current_tab == i as u32;
            let brush = if active { accent_brush } else { text_gray };

            let start_x = (i as f32) * col_width;
            let rect_btn = D2D_RECT_F {
                left: start_x,
                top: nav_y + 10.0,
                right: start_x + col_width,
                bottom: nav_y + 60.0,
            };

            unsafe {
                // If active, draw indicator bar
                if active {
                    rt.DrawLine(
                        D2D_POINT_2F { x: start_x + 30.0, y: nav_y },
                        D2D_POINT_2F { x: start_x + col_width - 30.0, y: nav_y },
                        accent_brush,
                        3.0,
                        None,
                    );
                }

                let _ = self.fmt_body_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                let _ = self.fmt_body_bold.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                self.draw_text(rt, tabs[i], &self.fmt_body_bold, &rect_btn, brush);
            }
        }
    }

    fn draw_toggle_switch(&self, rt: &ID2D1HwndRenderTarget, x: f32, y: f32, enabled: bool) {
        let accent_brush = self.br_accent.as_ref().unwrap();
        let border_brush = self.br_border.as_ref().unwrap();
        let bg_brush = self.br_bg.as_ref().unwrap();

        let width = 50.0;
        let height = 26.0;

        let r_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            },
            radiusX: 13.0,
            radiusY: 13.0,
        };

        unsafe {
            if enabled {
                rt.FillRoundedRectangle(&r_rect, accent_brush);
                rt.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: x + width - 13.0, y: y + 13.0 },
                        radiusX: 10.0,
                        radiusY: 10.0,
                    },
                    self.br_text_white.as_ref().unwrap(),
                );
            } else {
                rt.FillRoundedRectangle(&r_rect, bg_brush);
                rt.DrawRoundedRectangle(&r_rect, border_brush, 1.5, None);
                rt.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: x + 13.0, y: y + 13.0 },
                        radiusX: 10.0,
                        radiusY: 10.0,
                    },
                    accent_brush,
                );
            }
        }
    }

    fn draw_small_toggle_switch(&self, rt: &ID2D1HwndRenderTarget, x: f32, y: f32, enabled: bool) {
        let accent_brush = self.br_accent.as_ref().unwrap();
        let border_brush = self.br_border.as_ref().unwrap();
        let bg_brush = self.br_bg.as_ref().unwrap();

        let width = 44.0;
        let height = 22.0;

        let r_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            },
            radiusX: 11.0,
            radiusY: 11.0,
        };

        unsafe {
            if enabled {
                rt.FillRoundedRectangle(&r_rect, accent_brush);
                rt.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: x + width - 11.0, y: y + 11.0 },
                        radiusX: 8.0,
                        radiusY: 8.0,
                    },
                    self.br_text_white.as_ref().unwrap(),
                );
            } else {
                rt.FillRoundedRectangle(&r_rect, bg_brush);
                rt.DrawRoundedRectangle(&r_rect, border_brush, 1.5, None);
                rt.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: x + 11.0, y: y + 11.0 },
                        radiusX: 8.0,
                        radiusY: 8.0,
                    },
                    accent_brush,
                );
            }
        }
    }

    fn draw_progress_ring(
        &self,
        rt: &ID2D1HwndRenderTarget,
        cx: f32,
        cy: f32,
        r: f32,
        percent: f32,
        stroke_width: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        if percent <= 0.0 {
            return Ok(());
        }

        if percent >= 0.999 {
            unsafe {
                rt.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: cx, y: cy },
                        radiusX: r,
                        radiusY: r,
                    },
                    brush,
                    stroke_width,
                    None,
                );
            }
            return Ok(());
        }

        unsafe {
            let path_geometry = self.d2d_factory.CreatePathGeometry()?;
            let sink = path_geometry.Open()?;

            let start_point = D2D_POINT_2F { x: cx, y: cy - r };
            sink.BeginFigure(start_point, D2D1_FIGURE_BEGIN_HOLLOW);

            let angle = percent * 360.0;
            let rad = (angle - 90.0) * std::f32::consts::PI / 180.0;
            let end_point = D2D_POINT_2F {
                x: cx + r * rad.cos(),
                y: cy + r * rad.sin(),
            };

            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end_point,
                size: D2D_SIZE_F { width: r, height: r },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if angle > 180.0 { D2D1_ARC_SIZE_LARGE } else { D2D1_ARC_SIZE_SMALL },
            });

            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;

            rt.DrawGeometry(&path_geometry, brush, stroke_width, None);
        }

        Ok(())
    }

    // Handles mouse clicks inside the GUI window
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) {
        let mut state = self.state.lock().unwrap();
        let fx = x as f32;
        let fy = y as f32;

        // 1. Check Bottom Tab Navigation clicks (always absolute, ignore scroll)
        let nav_y = 560.0;
        if fy >= nav_y && fy <= nav_y + 80.0 {
            let col_width = 420.0 / 2.0;
            let tab_clicked = (fx / col_width) as u32;
            if tab_clicked < 2 && state.current_tab != tab_clicked {
                state.current_tab = tab_clicked;
                unsafe { InvalidateRect(self.hwnd, None, true) };
            }
            return;
        }

        // Header active toggle (Y: 25 to 75 is fixed, check screen fy, works on all tabs!)
        if fx >= 330.0 && fx <= 380.0 && fy >= 25.0 && fy <= 55.0 {
            state.config.enabled = !state.config.enabled;
            state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
            state.total_seconds = state.config.interval_mins * 60;
            state.is_dirty = true;
            crate::tray::update_tray_status(self.hwnd, state.config.enabled);
            unsafe { InvalidateRect(self.hwnd, None, true) };
            return;
        }

        // 2. Handle clicks inside active tab view
        match state.current_tab {
            0 => { // Dashboard (Supports Scrolling!)
                let scroll_y = state.dashboard_scroll as f32;
                let clicked_fy = fy + scroll_y; // Map screen Y to scroll content Y

                // If quiet hours enabled, check rule cards clicks
                if state.config.quiet_hours_enabled {
                    let inner_y_start = 300.0 + 70.0;
                    let rule_height = 185.0;
                    let spacing = 15.0;

                    for i in 0..state.quiet_hours_rules.len() {
                        let y_offset = inner_y_start + (i as f32 * (rule_height + spacing));
                        
                        // Check Rule Enable toggle (x: 50..94, y: y_offset + 15..y_offset + 37)
                        if fx >= 50.0 && fx <= 94.0 && clicked_fy >= y_offset + 15.0 && clicked_fy <= y_offset + 37.0 {
                            state.quiet_hours_rules[i].enabled = !state.quiet_hours_rules[i].enabled;
                            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                            state.config.quiet_hours_rules = rules_str.join(";");
                            state.is_dirty = true;
                            unsafe { InvalidateRect(self.hwnd, None, true) };
                            return;
                        }

                        // Check Preset Dropdown selection (x: 105..215, y: y_offset + 13..y_offset + 39)
                        if fx >= 105.0 && fx <= 215.0 && clicked_fy >= y_offset + 13.0 && clicked_fy <= y_offset + 39.0 {
                            // Release state lock to show TrackPopupMenu without block lock
                            drop(state);
                            self.show_preset_menu(i);
                            return;
                        }

                        // Check Delete rule button (x: 350..375, y: y_offset + 10..y_offset + 35)
                        if fx >= 350.0 && fx <= 375.0 && clicked_fy >= y_offset + 10.0 && clicked_fy <= y_offset + 35.0 {
                            state.quiet_hours_rules.remove(i);
                            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                            state.config.quiet_hours_rules = rules_str.join(";");
                            state.is_dirty = true;
                            unsafe { InvalidateRect(self.hwnd, None, true) };
                            return;
                        }

                        // Check Day Circles (Mon..Sun = 0..6)
                        if state.quiet_hours_rules[i].preset == "custom" {
                            for d in 0..7 {
                                let cx_day = 55.0 + d as f32 * 42.0;
                                let cy_day = y_offset + 92.0;
                                let dist = ((fx - cx_day).powi(2) + (clicked_fy - cy_day).powi(2)).sqrt();
                                if dist <= 14.0 {
                                    state.quiet_hours_rules[i].days[d] = !state.quiet_hours_rules[i].days[d];
                                    let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                                    state.config.quiet_hours_rules = rules_str.join(";");
                                    state.is_dirty = true;
                                    unsafe { InvalidateRect(self.hwnd, None, true) };
                                    return;
                                }
                            }
                        }

                        // Check Slider Double thumbs
                        if clicked_fy >= y_offset + 120.0 && clicked_fy <= y_offset + 150.0 && fx >= 40.0 && fx <= 360.0 {
                            let rule = &state.quiet_hours_rules[i];
                            let x_start = 50.0 + (rule.start_hour as f32 / 23.0) * 300.0;
                            let x_end = 50.0 + (rule.end_hour as f32 / 23.0) * 300.0;

                            let dist_start = (fx - x_start).abs();
                            let dist_end = (fx - x_end).abs();

                            state.active_rule_index = Some(i);
                            if dist_start < dist_end {
                                state.dragging_start_hour = true;
                            } else {
                                state.dragging_end_hour = true;
                            }
                            return;
                        }

                        // Check Overnight Toggle Switch
                        if fx >= 320.0 && fx <= 364.0 && clicked_fy >= y_offset + 158.0 && clicked_fy <= y_offset + 180.0 {
                            state.quiet_hours_rules[i].overnight = !state.quiet_hours_rules[i].overnight;
                            
                            // Swap start/end hours to transition the range selection logically
                            let r = &mut state.quiet_hours_rules[i];
                            std::mem::swap(&mut r.start_hour, &mut r.end_hour);

                            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                            state.config.quiet_hours_rules = rules_str.join(";");
                            state.is_dirty = true;
                            unsafe { InvalidateRect(self.hwnd, None, true) };
                            return;
                        }
                    }

                    // Check Add Rule Button (+ Add Rule)
                    let add_btn_y = inner_y_start + (state.quiet_hours_rules.len() as f32 * (rule_height + spacing));
                    if fx >= 35.0 && fx <= 385.0 && clicked_fy >= add_btn_y && clicked_fy <= add_btn_y + 40.0 {
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
                        state.is_dirty = true;
                        unsafe { InvalidateRect(self.hwnd, None, true) };
                        return;
                    }
                }

                // Check Master Quiet Hours Switch click
                if fx >= 330.0 && fx <= 380.0 && clicked_fy >= 315.0 && clicked_fy <= 341.0 {
                    state.config.quiet_hours_enabled = !state.config.quiet_hours_enabled;
                    state.is_dirty = true;
                    unsafe { InvalidateRect(self.hwnd, None, true) };
                    return;
                }

                // Check if clicking scrollbar area
                if fx >= 400.0 && fx <= 420.0 && fy >= 100.0 && fy <= 540.0 {
                    state.scrollbar_dragging = true;
                    state.drag_start_y = fy;
                    state.drag_start_scroll = state.dashboard_scroll;
                    return;
                }

                // If not clicking any button, enable touch-scroll dragging
                if fy >= 85.0 && fy <= 560.0 {
                    state.dashboard_dragging = true;
                    state.drag_start_y = fy;
                    state.drag_start_scroll = state.dashboard_scroll;
                }
            }
            1 => { // Settings
                // Check Volume slider click/drag start
                if fy >= 135.0 && fy <= 155.0 && fx >= 30.0 && fx <= 390.0 {
                    state.volume_dragging = true;
                    let pct = ((fx - 40.0) / 340.0).clamp(0.0, 1.0);
                    // Snap to 5% steps (from 0 to 100)
                    state.config.volume = (pct * 20.0).round() as u32 * 5;
                    state.is_dirty = true;
                    unsafe { InvalidateRect(self.hwnd, None, true) };
                }

                // Check Interval slider click/drag start
                if fy >= 240.0 && fy <= 260.0 && fx >= 30.0 && fx <= 390.0 {
                    state.interval_dragging = true;
                    let pct = ((fx - 40.0) / 340.0).clamp(0.0, 1.0);
                    // Snap to 5-minute steps (from 5 to 60)
                    let steps = (5.0 + (pct * 55.0)) / 5.0;
                    state.config.interval_mins = steps.round() as u32 * 5;
                    state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
                    state.total_seconds = state.config.interval_mins * 60;
                    state.is_dirty = true;
                    unsafe { InvalidateRect(self.hwnd, None, true) };
                }

                // Check Auto-Start Toggle Switch (Y: 300 to 360)
                if fx >= 330.0 && fx <= 380.0 && fy >= 315.0 && fy <= 341.0 {
                    state.config.run_at_startup = !state.config.run_at_startup;
                    state.is_dirty = true;
                    let _ = set_run_at_startup(state.config.run_at_startup);
                    unsafe { InvalidateRect(self.hwnd, None, true) };
                }
            }
            _ => {}
        }
    }

    pub fn handle_mouse_move(&mut self, x: i32, y: i32) {
        let mut state = self.state.lock().unwrap();
        let fx = x as f32;
        let fy = y as f32;

        if state.current_tab == 1 && state.volume_dragging {
            let start_x = 40.0;
            let width = 340.0;
            let pct = ((fx - start_x) / width).clamp(0.0, 1.0);
            // Snap to 5% steps (from 0 to 100)
            state.config.volume = (pct * 20.0).round() as u32 * 5;
            state.is_dirty = true;
            unsafe { InvalidateRect(self.hwnd, None, true) };
        } else if state.current_tab == 1 && state.interval_dragging {
            let start_x = 40.0;
            let width = 340.0;
            let pct = ((fx - start_x) / width).clamp(0.0, 1.0);
            // Snap to 5-minute steps (from 5 to 60)
            let steps = (5.0 + (pct * 55.0)) / 5.0;
            state.config.interval_mins = steps.round() as u32 * 5;
            state.remaining_seconds = crate::get_seconds_until_next_boundary(state.config.interval_mins);
            state.total_seconds = state.config.interval_mins * 60;
            state.is_dirty = true;
            unsafe { InvalidateRect(self.hwnd, None, true) };
        } else if state.current_tab == 0 && state.active_rule_index.is_some() {
            // Dragging start/end hour range slider thumb
            let rule_idx = state.active_rule_index.unwrap();
            let start_x = 50.0;
            let width = 300.0;
            let pct = ((fx - start_x) / width).clamp(0.0, 1.0);
            let hour = (pct * 23.0).round() as u32;

            if state.dragging_start_hour {
                state.quiet_hours_rules[rule_idx].start_hour = hour;
            } else if state.dragging_end_hour {
                state.quiet_hours_rules[rule_idx].end_hour = hour;
            }

            // Smart Auto-toggle Overnight spans midnight
            let r = &mut state.quiet_hours_rules[rule_idx];
            if r.start_hour > r.end_hour {
                r.overnight = true;
            } else {
                r.overnight = false;
            }

            let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
            state.config.quiet_hours_rules = rules_str.join(";");
            state.is_dirty = true;
            unsafe { InvalidateRect(self.hwnd, None, true) };
        } else if state.dashboard_dragging {
            // Scroll content
            let dy = fy - state.drag_start_y;
            
            let inner_y_start = 300.0 + 70.0;
            let card_qh_bottom = inner_y_start + (state.quiet_hours_rules.len() as f32 * 200.0) + 40.0 + 15.0;
            let total_content_height = if state.config.quiet_hours_enabled {
                card_qh_bottom + 20.0
            } else {
                300.0 + 130.0 + 20.0
            };
            let viewport_height = 475.0;
            let max_scroll = (total_content_height - viewport_height).max(0.0) as i32;

            state.dashboard_scroll = (state.drag_start_scroll - dy as i32).clamp(0, max_scroll);
            unsafe { InvalidateRect(self.hwnd, None, true) };
        } else if state.current_tab == 0 && state.scrollbar_dragging {
            let dy = fy - state.drag_start_y;
            
            let inner_y_start = 300.0 + 70.0;
            let card_qh_bottom = inner_y_start + (state.quiet_hours_rules.len() as f32 * 200.0) + 40.0 + 15.0;
            let total_content_height = if state.config.quiet_hours_enabled {
                card_qh_bottom + 20.0
            } else {
                300.0 + 130.0 + 20.0
            };
            let viewport_height = 475.0;
            let max_scroll = (total_content_height - viewport_height).max(0.0) as i32;

            let track_height = 440.0; // 540 - 100
            let ratio = total_content_height / track_height;
            let scroll_diff = (dy * ratio) as i32;

            state.dashboard_scroll = (state.drag_start_scroll + scroll_diff).clamp(0, max_scroll);
            unsafe { InvalidateRect(self.hwnd, None, true) };
        }
    }

    pub fn handle_mouse_wheel(&mut self, delta: i16) {
        let mut state = self.state.lock().unwrap();
        if state.current_tab == 0 { // Dashboard
            let scroll_amount = (delta as f32 * 0.4) as i32;
            
            let inner_y_start = 300.0 + 70.0;
            let card_qh_bottom = inner_y_start + (state.quiet_hours_rules.len() as f32 * 200.0) + 40.0 + 15.0;
            let total_content_height = if state.config.quiet_hours_enabled {
                card_qh_bottom + 20.0
            } else {
                300.0 + 130.0 + 20.0
            };
            let viewport_height = 475.0;
            let max_scroll = (total_content_height - viewport_height).max(0.0) as i32;

            state.dashboard_scroll = (state.dashboard_scroll - scroll_amount).clamp(0, max_scroll);
            unsafe { InvalidateRect(self.hwnd, None, true) };
        }
    }

    pub fn handle_mouse_up(&mut self) {
        let mut state = self.state.lock().unwrap();
        if state.volume_dragging {
            state.volume_dragging = false;
            state.config.save_to_file(&state.config_path);
            state.is_dirty = false;
        }
        if state.interval_dragging {
            state.interval_dragging = false;
            state.config.save_to_file(&state.config_path);
            state.is_dirty = false;
        }
        if state.active_rule_index.is_some() {
            state.active_rule_index = None;
            state.dragging_start_hour = false;
            state.dragging_end_hour = false;
            state.config.save_to_file(&state.config_path);
            state.is_dirty = false;
        }
        if state.dashboard_dragging {
            state.dashboard_dragging = false;
        }
        if state.scrollbar_dragging {
            state.scrollbar_dragging = false;
        }
    }

    fn show_preset_menu(&self, rule_idx: usize) {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, AppendMenuW, MF_STRING, TrackPopupMenu, TPM_RETURNCMD, TPM_LEFTALIGN};
            use windows::Win32::Foundation::POINT;
            
            let hmenu = CreatePopupMenu().unwrap_or_default();
            let _ = AppendMenuW(hmenu, MF_STRING, 1000, w!("Every Day"));
            let _ = AppendMenuW(hmenu, MF_STRING, 1001, w!("Work Days"));
            let _ = AppendMenuW(hmenu, MF_STRING, 1002, w!("Weekends"));
            let _ = AppendMenuW(hmenu, MF_STRING, 1003, w!("Custom"));
            
            let mut pt = POINT::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
            
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_RETURNCMD | TPM_LEFTALIGN,
                pt.x,
                pt.y,
                0,
                self.hwnd,
                None,
            );
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(hmenu);
            
            if cmd.0 > 0 {
                let mut state = self.state.lock().unwrap();
                if rule_idx < state.quiet_hours_rules.len() {
                    let rule = &mut state.quiet_hours_rules[rule_idx];
                    match cmd.0 {
                        1000 => {
                            rule.preset = "every_day".to_string();
                            rule.days = [true; 7];
                        }
                        1001 => {
                            rule.preset = "work_days".to_string();
                            rule.days = [true, true, true, true, true, false, false];
                        }
                        1002 => {
                            rule.preset = "weekends".to_string();
                            rule.days = [false, false, false, false, false, true, true];
                        }
                        1003 => {
                            rule.preset = "custom".to_string();
                        }
                        _ => {}
                    }
                    let rules_str: Vec<String> = state.quiet_hours_rules.iter().map(|r| r.serialize()).collect();
                    state.config.quiet_hours_rules = rules_str.join(";");
                    state.is_dirty = true;
                    let _ = InvalidateRect(self.hwnd, None, true);
                }
            }
        }
    }
}

pub fn set_run_at_startup(enabled: bool) -> Result<()> {
    unsafe {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY_CURRENT_USER, REG_SZ, KEY_WRITE, REG_OPTION_NON_VOLATILE
        };
        use std::os::windows::ffi::OsStrExt;

        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey,
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        ).ok()?;
        
        let value_name = w!("AutoZikr");
        
        if enabled {
            if let Ok(exe_path) = std::env::current_exe() {
                let mut path_utf16: Vec<u16> = exe_path.as_os_str().encode_wide().collect();
                path_utf16.push(0);
                
                RegSetValueExW(
                    hkey,
                    value_name,
                    0,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        path_utf16.as_ptr() as *const u8,
                        path_utf16.len() * 2,
                    )),
                ).ok()?;
            }
        } else {
            let res = RegDeleteValueW(hkey, value_name);
            // Ignore if file not found (error code 2)
            if res.is_err() && res.0 != 2 {
                let _ = RegCloseKey(hkey);
                return Err(res.into());
            }
        }
        
        let _ = RegCloseKey(hkey);
        Ok(())
    }
}

// Global window callback
pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_SETTINGCHANGE => {
            // Update tray icon dynamically
            crate::tray::update_tray_icon_theme(hwnd);
            // Repaint GUI window for new colors
            windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, true);
            LRESULT(0)
        }
        crate::tray::WM_TRAY_ICON => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                (*ctx_ptr).handle_tray_icon(lparam);
            }
            LRESULT(0)
        }
        crate::tray::WM_ALREADY_RUNNING => {
            crate::tray::show_tray_notification(
                hwnd,
                "AutoZikr",
                "AutoZikr is already running in your system tray.",
            );
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                (*ctx_ptr).handle_command(wparam);
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_ACTIVATE => {
            let active = (wparam.0 & 0xFFFF) as u16;
            if active == 0 { // WA_INACTIVE
                let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
                if !ctx_ptr.is_null() {
                    (*ctx_ptr).last_hide_time.set(std::time::Instant::now());
                }
                windows::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_HIDE);
            }
            LRESULT(0)
        }
        WM_CREATE => {
            let create_struct = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            let state_raw = create_struct.lpCreateParams as *const Mutex<AppState>;
            
            // Reconstruct Arc from the raw pointer passed during creation
            let state = Arc::from_raw(state_raw);
            let state_clone = Arc::clone(&state);
            // Put it back to raw pointer so it doesn't get dropped when going out of scope
            let _ = Arc::into_raw(state);
            
            // Box and store GUI context
            let ctx = Box::new(GuiContext::new(hwnd, state_clone));
            let ctx_ptr = Box::into_raw(ctx);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx_ptr as isize);
            
            LRESULT(0)
        }
        WM_PAINT => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                let mut ps = PAINTSTRUCT::default();
                let _hdc = BeginPaint(hwnd, &mut ps);
                (*ctx_ptr).paint();
                EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                (*ctx_ptr).discard_resources();
                InvalidateRect(hwnd, None, true);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                (*ctx_ptr).handle_mouse_down(x, y);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                (*ctx_ptr).handle_mouse_move(x, y);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                (*ctx_ptr).handle_mouse_up();
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_MOUSEWHEEL => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
                (*ctx_ptr).handle_mouse_wheel(delta);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ctx_ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut GuiContext;
            if !ctx_ptr.is_null() {
                let _ctx = Box::from_raw(ctx_ptr);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Query current Windows App Theme setting from Registry
pub fn is_dark_mode() -> bool {
    unsafe {
        use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ};
        use windows::core::w;
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut hkey).is_ok() {
            let mut data = 0u32;
            let mut data_len = std::mem::size_of::<u32>() as u32;
            let res = RegQueryValueExW(
                hkey,
                w!("AppsUseLightTheme"),
                None,
                None,
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut data_len),
            );
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
            if res.is_ok() {
                return data == 0; // 0 is Dark, 1 is Light
            }
        }
    }
    true // fallback to Dark Theme
}

/// Query current Windows Taskbar/System Theme setting from Registry
pub fn is_system_dark_mode() -> bool {
    unsafe {
        use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ};
        use windows::core::w;
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut hkey).is_ok() {
            let mut data = 0u32;
            let mut data_len = std::mem::size_of::<u32>() as u32;
            let mut res = RegQueryValueExW(
                hkey,
                w!("SystemUsesLightTheme"),
                None,
                None,
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut data_len),
            );
            if res.is_err() {
                res = RegQueryValueExW(
                    hkey,
                    w!("AppsUseLightTheme"),
                    None,
                    None,
                    Some(&mut data as *mut u32 as *mut u8),
                    Some(&mut data_len),
                );
            }
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
            if res.is_ok() {
                return data == 0; // 0 is Dark, 1 is Light
            }
        }
    }
    true // fallback to Dark Theme
}