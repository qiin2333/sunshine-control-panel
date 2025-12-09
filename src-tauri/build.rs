fn main() {
    // 告诉 Cargo 当这些文件变化时需要重新编译
    // 这样 include_str! 宏就能读取到最新版本的文件
    println!("cargo:rerun-if-changed=inject-script.js");
    println!("cargo:rerun-if-changed=inject-styles.css");
    println!("cargo:rerun-if-changed=error-404.html");
    
    tauri_build::build()
}


