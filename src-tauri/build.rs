fn main() {
    tauri_build::build();

    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new("/Applications/Xcode.app").exists() {
            std::env::set_var(
                "DEVELOPER_DIR",
                "/Applications/Xcode.app/Contents/Developer",
            );
            println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
            println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
        }
        swift_rs::SwiftLinker::new("14.0")
            .with_package("OpenEditorMedia", "../native/OpenEditorMedia")
            .link();
    }
}
