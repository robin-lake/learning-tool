//! The repo may live on a non-Apple filesystem (e.g. exFAT) where macOS writes `._*`
//! AppleDouble sidecar files next to every file. `sqlx::migrate!` rejects those names,
//! so remove them from the migrations directory before the macro reads it.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
    if let Ok(entries) = std::fs::read_dir("migrations") {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("._") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}
