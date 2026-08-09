use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    NOTIFYICONDATAW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIF_INFO, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY,
};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, SetForegroundWindow, TrackPopupMenu,
    HICON, IDI_APPLICATION, MF_STRING, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_USER,
};

pub const WM_TRAY_ICON: u32 = WM_USER + 1;
pub const WM_ALREADY_RUNNING: u32 = WM_USER + 100;
pub const TRAY_ICON_ID: u32 = 1;

pub const ID_TRAY_OPEN: u32 = 1001;
pub const ID_TRAY_TOGGLE: u32 = 1002;
pub const ID_TRAY_EXIT: u32 = 1003;

/// Adds the icon to the system tray.
pub fn add_tray_icon(hwnd: HWND) -> bool {
    unsafe {
        let is_dark = crate::gui::is_system_dark_mode();
        let icon_id = if is_dark { 2 } else { 3 };
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
        let hicon = LoadIconW(hinstance, PCWSTR(icon_id as *const u16)).unwrap_or_else(|_| {
            LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON::default())
        });
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: hicon,
            ..Default::default()
        };

        // Set initial tooltip text
        let tip = w!("AutoZikr - Reminders");
        let len = tip.len().min(nid.szTip.len() - 1);
        std::ptr::copy_nonoverlapping(tip.as_ptr(), nid.szTip.as_mut_ptr(), len);
        nid.szTip[len] = 0; // null terminate

        Shell_NotifyIconW(NIM_ADD, &nid).as_bool()
    }
}

/// Updates the tray icon tooltip based on active status.
pub fn update_tray_status(hwnd: HWND, enabled: bool) -> bool {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_TIP,
            ..Default::default()
        };

        let tip = if enabled {
            w!("AutoZikr - Active")
        } else {
            w!("AutoZikr - Reminders are Off")
        };
        let len = tip.len().min(nid.szTip.len() - 1);
        std::ptr::copy_nonoverlapping(tip.as_ptr(), nid.szTip.as_mut_ptr(), len);
        nid.szTip[len] = 0; // null terminate

        Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool()
    }
}

/// Removes the icon from the system tray.
pub fn remove_tray_icon(hwnd: HWND) -> bool {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        Shell_NotifyIconW(NIM_DELETE, &nid).as_bool()
    }
}

type SetPreferredAppModeFn = unsafe extern "system" fn(i32) -> i32;
type AllowDarkModeForWindowFn = unsafe extern "system" fn(HWND, bool) -> bool;
type FlushMenuThemesFn = unsafe extern "system" fn();

pub fn enable_dark_mode_menus(hwnd: HWND, is_dark: bool) {
    unsafe {
        use windows::core::PCSTR;
        use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
        use windows::Win32::UI::Controls::SetWindowTheme;

        if let Ok(hmod) = LoadLibraryA(PCSTR(b"uxtheme.dll\0".as_ptr())) {
            let mode = if is_dark { 2 } else { 3 }; // 2 = ForceDark, 3 = ForceLight
            
            if let Some(set_preferred_app_mode) = GetProcAddress(hmod, PCSTR(135 as *const u8)) {
                let func: SetPreferredAppModeFn = std::mem::transmute(set_preferred_app_mode);
                func(mode);
            }
            
            if let Some(allow_dark_mode) = GetProcAddress(hmod, PCSTR(133 as *const u8)) {
                let func: AllowDarkModeForWindowFn = std::mem::transmute(allow_dark_mode);
                func(hwnd, is_dark);
            }

            if let Some(flush_menu_themes) = GetProcAddress(hmod, PCSTR(136 as *const u8)) {
                let func: FlushMenuThemesFn = std::mem::transmute(flush_menu_themes);
                func();
            }
        }

        let theme = if is_dark { w!("DarkMode_Explorer") } else { w!("Explorer") };
        let _ = SetWindowTheme(hwnd, PCWSTR(theme.as_ptr()), None);
    }
}

/// Displays the right-click popup context menu for the system tray icon.
pub fn show_context_menu(hwnd: HWND, enabled: bool) {
    unsafe {
        let is_dark = crate::gui::is_system_dark_mode();
        enable_dark_mode_menus(hwnd, is_dark);

        let menu = CreatePopupMenu().unwrap();
        
        let open_text = w!("Open Dashboard");
        let toggle_text = if enabled {
            w!("Mute Reminders")
        } else {
            w!("Unmute Reminders")
        };
        let exit_text = w!("Exit");

        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_OPEN as usize, PCWSTR(open_text.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_TOGGLE as usize, PCWSTR(toggle_text.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_EXIT as usize, PCWSTR(exit_text.as_ptr()));

        let mut pos = POINT::default();
        let _ = GetCursorPos(&mut pos);

        // Set the window as foreground before displaying context menu to ensure it closes when clicking outside
        let _ = SetForegroundWindow(hwnd);
        
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON,
            pos.x,
            pos.y,
            0,
            hwnd,
            None,
        );

        let _ = DestroyMenu(menu);
    }
}

/// Updates system tray icon theme dynamically when system light/dark settings change
pub fn update_tray_icon_theme(hwnd: HWND) {
    let is_dark = crate::gui::is_system_dark_mode();
    enable_dark_mode_menus(hwnd, is_dark);
    let icon_id = if is_dark { 2 } else { 3 };
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
        let hicon = LoadIconW(hinstance, PCWSTR(icon_id as *const u16)).unwrap_or_else(|_| {
            LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON::default())
        });
        
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON,
            hIcon: hicon,
            ..Default::default()
        };
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

/// Displays a system notification balloon near the system tray icon
pub fn show_tray_notification(hwnd: HWND, title: &str, message: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_INFO,
        ..Default::default()
    };

    let title_u16: Vec<u16> = OsStr::new(title).encode_wide().chain(std::iter::once(0)).collect();
    let msg_u16: Vec<u16> = OsStr::new(message).encode_wide().chain(std::iter::once(0)).collect();

    let title_len = title_u16.len().min(nid.szInfoTitle.len() - 1);
    nid.szInfoTitle[..title_len].copy_from_slice(&title_u16[..title_len]);

    let msg_len = msg_u16.len().min(nid.szInfo.len() - 1);
    nid.szInfo[..msg_len].copy_from_slice(&msg_u16[..msg_len]);

    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}