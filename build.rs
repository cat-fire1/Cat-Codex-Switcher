fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    winres::WindowsResource::new()
        .set_icon("assets/app.ico")
        .set("ProductName", "Cat Codex Switcher")
        .set("FileDescription", "Cat Codex Switcher — Rust Windows application")
        .set("LegalCopyright", "Based on Codex Account Switcher (c) 2026 liuzhao1225, MIT")
        .compile()
        .expect("Windows SDK resource compiler is required");
}
