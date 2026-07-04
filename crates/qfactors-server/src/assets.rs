//! The built frontend, embedded into the binary when present at compile time
//! (see `build.rs`, which sets `cfg(have_assets)`). Without it the server runs
//! API-only.

#[cfg(have_assets)]
use include_dir::{Dir, include_dir};

#[cfg(have_assets)]
static DIST: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../frontend/dist");

#[cfg(have_assets)]
pub const HAVE: bool = true;
#[cfg(not(have_assets))]
pub const HAVE: bool = false;

#[cfg(have_assets)]
pub fn get(path: &str) -> Option<&'static [u8]> {
    DIST.get_file(path).map(|f| f.contents())
}

#[cfg(not(have_assets))]
pub fn get(_path: &str) -> Option<&'static [u8]> {
    None
}
