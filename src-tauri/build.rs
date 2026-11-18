fn main() {
    // 图标变化时强制重跑：tauri-build 不跟踪 icons/*.ico 的变化，
    // 只改图标不改 Rust 代码会导致 exe 嵌的还是旧图标（踩过）。
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/32x32.png");
    println!("cargo:rerun-if-changed=icons/128x128.png");
    println!("cargo:rerun-if-changed=icons/128x128@2x.png");
    tauri_build::build()
}
