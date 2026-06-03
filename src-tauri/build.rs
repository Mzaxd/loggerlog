#[cfg(feature = "gui")]
fn main() {
    tauri_build::build()
}

#[cfg(not(feature = "gui"))]
fn main() {
    // No-op for CLI-only builds
}
