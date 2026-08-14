fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/Stow.ico");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile windows resource icon: {}", e);
        }
    }
}
