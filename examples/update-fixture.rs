// Harmless executable used only by the native portable-updater test.
fn main() {
    let directory = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(directory.join("updated.marker"), "SIGNED-UPDATE-OK").unwrap();
}
