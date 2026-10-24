fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    winres::WindowsResource::new()
        .set_icon("assets/app.ico")
        .set("ProductName", "Cat Codex Switcher")
        .set("FileDescription", "Cat Codex Switcher")
        .set("LegalCopyright", "")
        .compile()
        .expect("Windows SDK resource compiler is required");
}
