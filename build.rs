//! aSynthe - Build

#[cfg(target_os="windows")]
use winres::WindowsResourec;


#[cfg(target_os="windows")]
fn main() {
    let mut res = WindowsResource::new();
    res.set_icon("test.ico");
    res.compile().unwrap();
}

fn main() {}