use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Re-run if build.rs, zikr_audio, or app_icon.ico changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=zikr_audio");
    println!("cargo:rerun-if-changed=app_icon.ico");
    println!("cargo:rerun-if-changed=app_icon_dark.ico");
    println!("cargo:rerun-if-changed=app_icon_light.ico");
    println!("cargo:rerun-if-changed=app.rc");

    let out_dir = env::var("OUT_DIR").unwrap();
    // Go up 3 levels from target/.../build/autozikr-hash/out to get target/...
    let profile_dir = Path::new(&out_dir)
        .parent().unwrap()
        .parent().unwrap()
        .parent().unwrap();

    let src_dir = Path::new("zikr_audio");
    if src_dir.exists() {
        let dest_dir = profile_dir.join("zikr_audio");
        let _ = fs::create_dir_all(&dest_dir);
        
        if let Ok(entries) = fs::read_dir(src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name() {
                        let dest_file = dest_dir.join(filename);
                        let _ = fs::copy(&path, &dest_file);
                    }
                }
            }
        }
    }

    // Embed executable icon if building for Windows
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_resource_file("app.rc");
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile resource file: {}", e);
            std::process::exit(1);
        }
    }
}
