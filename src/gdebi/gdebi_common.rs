//! Common package operations shared by the GTK and command-line frontends.
//!
//! This is the Rust equivalent of the original `GDebiCommon.py` module.  It
//! deliberately contains no GTK code, so package inspection is usable from a
//! terminal and remains unprivileged.

use super::deb_package::{DebPackage, PackageInfo};
use crate::i18n::tr;
use std::path::Path;

pub struct GDebiCommon;

impl GDebiCommon {
    pub fn open(path: impl AsRef<Path>) -> Result<DebPackage, String> {
        PackageInfo::from_path(path)
    }

    pub fn dependency_summary(package: &PackageInfo) -> String {
        let dependencies = package.dependencies();
        if dependencies.is_empty() {
            tr("All declared dependencies are satisfied, or this package has no dependencies.")
        } else {
            dependencies
                .into_iter()
                .map(|(label, value)| format!("{}: {value}", tr(label)))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}
