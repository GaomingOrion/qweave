use std::path::Path;

// Embed the built frontend only when it exists, so a plain `cargo build` of the
// workspace never requires `npm run build` first. Building the Python wheel
// (which serves the embedded UI) does need the frontend built beforehand.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(have_assets)");
    println!("cargo:rerun-if-changed=../../frontend/dist/index.html");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let index = Path::new(&manifest).join("../../frontend/dist/index.html");
    if index.is_file() {
        println!("cargo:rustc-cfg=have_assets");
    }
}
