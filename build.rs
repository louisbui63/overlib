fn main() {
    std::env::set_current_dir("./dlsym_hook").unwrap();
    std::process::Command::new("cargo")
        .arg("build")
        .status()
        .unwrap();
    std::env::set_current_dir("..").unwrap();
    println!("cargo:rustc-link-search=native=./real_dlsym");
    println!("cargo:rustc-link-lib=rdlsym");
}
