fn main() {
    println!("cargo:rustc-link-search=native=/home/louisbui63/dev/overlay/real_dlsym");
    println!("cargo:rustc-link-lib=rdlsym");
}
