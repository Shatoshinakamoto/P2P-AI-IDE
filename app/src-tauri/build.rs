//! build.rs -- generates the Tauri context (config, capabilities) at build time. No network or outside input.
fn main() {
    tauri_build::build()
}
