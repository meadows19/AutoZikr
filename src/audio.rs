use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::Media::Audio::{
    IMMDeviceEnumerator, MMDeviceEnumerator, eRender, eConsole, PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT,
};
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;

// Custom binding to MCI send string function in winmm.dll
#[link(name = "winmm")]
extern "system" {
    fn mciSendStringW(
        lpstrcommand: PCWSTR,
        lpstrreturnstring: windows::core::PWSTR,
        uformat: u32,
        hwndcallback: HWND,
    ) -> u32;
}

/// Plays WAV audio bytes directly from memory asynchronously (0ms latency, zero disk operations).
pub fn play_sound_bytes(data: &'static [u8], _volume: u32) {
    let addr = data.as_ptr() as usize;
    std::thread::spawn(move || {
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(addr as *const u16),
                None,
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    });
}

fn send_mci_command(cmd: &str) -> Result<(), u32> {
    let mut cmd_utf16: Vec<u16> = OsStr::new(cmd).encode_wide().collect();
    cmd_utf16.push(0); // null terminator
    let res = unsafe {
        mciSendStringW(
            PCWSTR(cmd_utf16.as_ptr()),
            windows::core::PWSTR::null(),
            0,
            HWND::default(),
        )
    };
    if res == 0 {
        Ok(())
    } else {
        Err(res)
    }
}

/// Plays a WAV file asynchronously in a separate thread.
/// Takes file path and volume (0 to 100).
pub fn play_sound(path: &Path, volume: u32) {
    let path_str = path.to_string_lossy();
    
    // We use an atomic counter to generate a unique alias for each playing sound.
    // This allows multiple sound playbacks (e.g. previewing and timer trigger) to run concurrently.
    static ALIAS_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let alias_id = ALIAS_COUNTER.fetch_add(1, Ordering::Relaxed);
    let alias = format!("zp_{}", alias_id);

    // MCI commands require paths with spaces to be quoted
    let open_cmd = format!("open \"{}\" type waveaudio alias {}", path_str, alias);
    let vol_cmd = format!("setaudio {} volume to {}", alias, volume * 10); // MCI volume is 0..1000
    let play_cmd = format!("play {} wait", alias);
    let close_cmd = format!("close {}", alias);

    // Spawn a thread to handle blocking playback
    std::thread::spawn(move || {
        if send_mci_command(&open_cmd).is_ok() {
            let _ = send_mci_command(&vol_cmd);
            let _ = send_mci_command(&play_cmd);
            let _ = send_mci_command(&close_cmd);
        }
    });
}

/// Queries Windows Core Audio endpoint to check if the system is currently outputting sound.
/// Returns true if the default playback device's peak audio level is above a threshold.
pub fn is_audio_playing() -> bool {
    unsafe {
        match CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) {
            Ok(enumerator) => {
                match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                    Ok(device) => {
                        match device.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None) {
                            Ok(meter) => {
                                match meter.GetPeakValue() {
                                    Ok(peak) => peak > 0.005, // 0.5% volume threshold to filter out noise
                                    Err(_) => false,
                                }
                            }
                            Err(_) => false,
                        }
                    }
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }
}
