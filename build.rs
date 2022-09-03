//! aSynthe - Build

#[cfg(target_os="windows")]
use winres::WindowsResource;


#[cfg(target_os="windows")]
fn main() {
    let mut res = WindowsResource::new();
    res.set_icon("static/main.ico");
    res.compile().unwrap();
}

#[cfg(target_os="macos")]
fn main() {}