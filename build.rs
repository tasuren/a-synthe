//! aSynthe - Build

#[cfg(target_os = "windows")]
use tauri_winres::WindowsResource;

#[cfg(target_os = "windows")]
fn main() {
    let mut res = WindowsResource::new();
    res.set_icon("release/icon/main.ico");
    res.compile().unwrap();
}

#[cfg(target_os = "macos")]
fn main() {}
