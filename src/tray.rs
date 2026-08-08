use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    NOTIFYICONDATAW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, SetForegroundWindow, TrackPopupMenu,
    HICON, IDI_APPLICATION, MF_STRING, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_USER,
};

pub const WM_TRAY_ICON: u32 = WM_USER + 1;
pub const TRAY_ICON_ID: u32 = 1;

pub const ID_TRAY_OPEN: u32 = 1001;
pub const ID_TRAY_TOGGLE: u32 = 1002;
pub const ID_TRAY_EXIT: u32 = 1003;

/// Adds the icon to the system tray.
pub fn add_tray_icon(hwnd: HWND) -> bool {
    unsafe {
        let is_dark = crate::gui::is_system_dark_mode();
        let icon_id = if is_dark { 2 } else { 1 };
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

/// Displays the right-click popup context menu for the system tray icon.
pub fn show_context_menu(hwnd: HWND, enabled: bool) {
    unsafe {
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
    let icon_id = if is_dark { 2 } else { 1 };
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