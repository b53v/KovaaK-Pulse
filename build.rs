fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "KovaaK Pulse");
        res.set(
            "FileDescription",
            "Lightweight real-time monitor and analytics GUI for KovaaK's",
        );
        if let Err(err) = res.compile() {
            panic!("failed to compile Windows icon resource: {err}");
        }
    }
}
