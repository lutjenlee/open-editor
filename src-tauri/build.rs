fn main() {
    tauri_build::build();

    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new("/Applications/Xcode.app").exists() {
            std::env::set_var("DEVELOPER_DIR", "/Applications/Xcode.app/Contents/Developer");
        }
        swift_rs::SwiftLinker::new("14.0")
            .with_package("OpenEditorMedia", "../native/OpenEditorMedia")
            .link();
    }
}
