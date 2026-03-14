fn main() {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=resources/taurhaus-daemon");
    println!("cargo:rerun-if-changed=resources/mesh");
    println!("cargo:rerun-if-changed=resources/mesh.version");
    println!("cargo:rerun-if-changed=resources/mesh.manifest.json");
    tauri_build::build()
}
