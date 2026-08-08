pub const BUILTIN_AUDIO_FILES: &[(&str, &[u8])] = &[
    ("4th Kalima.wav", include_bytes!("../audio/4th Kalima.wav")),
    ("Alhamdulillah.wav", include_bytes!("../audio/Alhamdulillah.wav")),
    ("Allah Allah.wav", include_bytes!("../audio/Allah Allah.wav")),
    ("Allah.wav", include_bytes!("../audio/Allah.wav")),
    ("Allahu Akbar.wav", include_bytes!("../audio/Allahu Akbar.wav")),
    ("Hawqala.wav", include_bytes!("../audio/Hawqala.wav")),
    ("Istigfar.wav", include_bytes!("../audio/Istigfar.wav")),
    ("Kalima.wav", include_bytes!("../audio/Kalima.wav")),
    ("Kalima2.wav", include_bytes!("../audio/Kalima2.wav")),
    ("Kalimatan.wav", include_bytes!("../audio/Kalimatan.wav")),
    ("Salawat.wav", include_bytes!("../audio/Salawat.wav")),
    ("SubhanAllah.wav", include_bytes!("../audio/SubhanAllah.wav")),
];

pub fn get_builtin_bytes(name: &str) -> Option<&'static [u8]> {
    for (n, bytes) in BUILTIN_AUDIO_FILES {
        if *n == name {
            return Some(bytes);
        }
    }
    None
}
