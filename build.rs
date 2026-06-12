#[cfg(target_os = "windows")]
fn main() {
    winresource::WindowsResource::new()
        .set_icon("assets/icon.ico")
        .compile()
        .expect("failed to embed Windows icon");
}

#[cfg(not(target_os = "windows"))]
fn main() {}
