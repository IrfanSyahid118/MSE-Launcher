fn main() {
    println!("cargo:rerun-if-env-changed=MCLAUNCHER_DEFAULT_CF_PROXY");
    println!("cargo:rerun-if-env-changed=CURSEFORGE_PROXY_URL");
    println!("cargo:rerun-if-env-changed=CURSEFORGE_API_KEY");
}
