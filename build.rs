fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_manifest_file("app.manifest");
        res.set("FileDescription", "Kid Internet Lock");
        res.set("ProductName", "Kid Internet Lock");
        res.set("LegalCopyright", "Copyright (c) 2026");
        if let Err(e) = res.compile() {
            eprintln!("winresource compile warning: {}", e);
        }
    }
}
