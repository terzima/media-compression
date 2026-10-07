fn main() {
    println!(
        "cargo:rustc-env=MEDIA_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    tauri_build::build()
}
