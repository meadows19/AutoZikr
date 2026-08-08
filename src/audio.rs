use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::Media::Audio::{
    IMMDeviceEnumerator, MMDeviceEnumerator, eRender, eConsole, PlaySoundW, SND_MEMORY, SND_NODEFAULT,
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

fn find_subchunk(data: &[u8], tag: &[u8; 4]) -> Option<usize> {
    if data.len() < 12 {
        return None;
    }
    let mut idx = 12; // Skip RIFF header
    while idx + 8 <= data.len() {
        if &data[idx..idx + 4] == tag {
            return Some(idx);
        }
        let chunk_size = u32::from_le_bytes([
            data[idx + 4],
            data[idx + 5],
            data[idx + 6],
            data[idx + 7],
        ]) as usize;
        idx += 8 + chunk_size;
    }
    None
}

/// Plays WAV audio bytes directly from memory asynchronously with real-time perceptual volume scaling.
pub fn play_sound_bytes(data: &'static [u8], volume: u32) {
    let linear_factor = (volume as f32 / 100.0).clamp(0.0, 1.0);
    // Cubic perceptual volume scaling: human hearing perceives sound logarithmically (decibels).
    // Linear amplitude scaling makes 20% volume sound almost identical to 100% volume.
    // Cubic scaling (linear^3) provides smooth, dramatic volume changes matching human ears.
    let vol_factor = linear_factor * linear_factor * linear_factor;
    
    let mut buf = data.to_vec();
    if vol_factor < 0.999 {
        if let Some(data_idx) = find_subchunk(&buf, b"data") {
            let sample_start = data_idx + 8;
            if sample_start < buf.len() {
                let samples_slice = &mut buf[sample_start..];
                for chunk in samples_slice.chunks_exact_mut(2) {
                    let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                    let scaled = (sample as f32 * vol_factor) as i16;
                    let le = scaled.to_le_bytes();
                    chunk[0] = le[0];
                    chunk[1] = le[1];
                }
            }
        }
    }

    std::thread::spawn(move || {
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(buf.as_ptr() as *const u16),
                None,
                SND_MEMORY | SND_NODEFAULT,
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

/// Plays a WAV file asynchronously from disk with real-time perceptual volume scaling.
pub fn play_sound(path: &Path, volume: u32) {
    if let Ok(bytes) = std::fs::read(path) {
        let linear_factor = (volume as f32 / 100.0).clamp(0.0, 1.0);
        let vol_factor = linear_factor * linear_factor * linear_factor;
        
        let mut buf = bytes;
        if vol_factor < 0.999 {
            if let Some(data_idx) = find_subchunk(&buf, b"data") {
                let sample_start = data_idx + 8;
                if sample_start < buf.len() {
                    let samples_slice = &mut buf[sample_start..];
                    for chunk in samples_slice.chunks_exact_mut(2) {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                        let scaled = (sample as f32 * vol_factor) as i16;
                        let le = scaled.to_le_bytes();
                        chunk[0] = le[0];
                        chunk[1] = le[1];
                    }
                }
            }
        }

        std::thread::spawn(move || {
            unsafe {
                let _ = PlaySoundW(
                    PCWSTR(buf.as_ptr() as *const u16),
                    None,
                    SND_MEMORY | SND_NODEFAULT,
                );
            }
        });
    }
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
