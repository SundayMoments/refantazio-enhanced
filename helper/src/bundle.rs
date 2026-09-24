use crate::storage::Package;
use anyhow::Result;
#[cfg(feature = "bundled")]
pub fn load() -> Result<Package> {
    Package::read(
        include_bytes!(env!("RE_HELPER_PAYLOAD")),
        include_str!(env!("RE_HELPER_MANIFEST")),
    )
}
#[cfg(not(feature = "bundled"))]
pub fn load() -> Result<Package> {
    anyhow::bail!("Development build: no bundled mod. Build with tools/build_helper.py.")
}
pub const AVAILABLE: bool = cfg!(feature = "bundled");
#[cfg(feature = "bundled")]
pub const NOTICES: &str = include_str!(env!("RE_HELPER_NOTICES"));
#[cfg(not(feature = "bundled"))]
pub const NOTICES: &str =
    "Development build. See CREDITS.md and licenses in the source repository.";
