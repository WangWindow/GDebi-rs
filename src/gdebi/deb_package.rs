//! Reading the bits of a Debian binary package that are useful to the UI.
//!
//! GDebi traditionally used python-apt for this job.  Keeping the metadata
//! reader independent from GTK makes it easier to test and, more importantly,
//! means that opening an untrusted `.deb` never requires administrator access.

use crate::i18n::tr;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub path: PathBuf,
    pub fields: BTreeMap<String, String>,
}

// Name used by the upstream GDebi module.  PackageInfo is retained as the
// descriptive Rust name used by the GTK state layer.
pub type DebPackage = PackageInfo;

impl PackageInfo {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!("{}: {}", tr("File does not exist"), path.display()));
        }
        if !path.is_file() {
            return Err(format!("{}: {}", tr("Not a regular file"), path.display()));
        }

        let path = path
            .canonicalize()
            .map_err(|error| format!("{}: {error}", tr("Unable to read file path")))?;
        let control = run_dpkg_deb(&["--field", path.to_string_lossy().as_ref()])?;
        let fields = parse_control_fields(&control);
        if !fields.contains_key("Package") || !fields.contains_key("Version") {
            return Err(tr(
                "This is not a valid Debian binary package (Package or Version is missing)",
            ));
        }

        Ok(Self { path, fields })
    }

    pub fn field(&self, name: &str) -> String {
        self.fields
            .get(name)
            .cloned()
            .unwrap_or_else(|| tr("Unknown"))
    }

    pub fn package_name(&self) -> String {
        self.field("Package")
    }

    pub fn version(&self) -> String {
        self.field("Version")
    }

    pub fn architecture(&self) -> String {
        self.field("Architecture")
    }

    pub fn description(&self) -> String {
        self.fields
            .get("Description")
            .map(|description| format_description(description))
            .unwrap_or_else(|| tr("No description is available."))
    }

    pub fn dependencies(&self) -> Vec<(&'static str, String)> {
        [
            ("Depends", "Depends"),
            ("Pre-Depends", "Pre-Depends"),
            ("Recommends", "Recommends"),
            ("Suggests", "Suggests"),
            ("Conflicts", "Conflicts"),
            ("Replaces", "Replaces"),
        ]
        .into_iter()
        .filter_map(|(field, label)| {
            self.fields
                .get(field)
                .filter(|value| !value.trim().is_empty())
                .map(|value| (label, value.clone()))
        })
        .collect()
    }
}

fn run_dpkg_deb(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("dpkg-deb")
        .args(arguments)
        .output()
        .map_err(|error| format!("{}: {error}", tr("Unable to run dpkg-deb")))?;

    if output.status.success() {
        return String::from_utf8(output.stdout)
            .map_err(|error| format!("{}: {error}", tr("dpkg-deb output is not valid UTF-8")));
    }

    let details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if details.is_empty() {
        Err(format!(
            "{}: {:?}",
            tr("dpkg-deb failed with exit status"),
            output.status.code()
        ))
    } else {
        Err(format!(
            "{}: {details}",
            tr("dpkg-deb could not open the package")
        ))
    }
}

/// Extract only an application icon from the data archive.
///
/// A compressed tar stream cannot provide true random access without an
/// external index. This helper therefore scans the archive once in a worker
/// thread and extracts only icon candidates, never the complete package.
pub fn extract_icon(package: &Path) -> Option<PathBuf> {
    let directory = std::env::temp_dir().join(format!(
        "gdebi-rs-icons-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    std::fs::create_dir_all(&directory).ok()?;

    let mut dpkg = Command::new("dpkg-deb")
        .args(["--fsys-tarfile", package.to_string_lossy().as_ref()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = dpkg.stdout.take()?;
    let patterns = [
        "./usr/share/icons/*/apps/*.png",
        "./usr/share/icons/*/apps/*.svg",
        "./usr/share/icons/*/apps/*.xpm",
        "./usr/share/pixmaps/*.png",
        "./usr/share/pixmaps/*.svg",
        "./usr/share/pixmaps/*.xpm",
    ];
    let tar_status = Command::new("tar")
        .args(["-x", "-f", "-", "-C"])
        .arg(&directory)
        .args(["--wildcards", "--no-anchored", "--no-same-owner"])
        .args(patterns)
        .stdin(Stdio::from(stdout))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?
        .wait()
        .ok()?;
    let _ = dpkg.wait();

    let mut candidates = Vec::new();
    collect_icon_files(&directory, &mut candidates);
    if !tar_status.success() && candidates.is_empty() {
        let _ = std::fs::remove_dir_all(&directory);
        return None;
    }
    candidates.sort_by_key(|path| {
        let lower = path.to_string_lossy().to_ascii_lowercase();
        let mut score = 0;
        if lower.contains("/apps/") {
            score -= 20;
        }
        if lower.contains("scalable") {
            score -= 10;
        }
        if lower.ends_with(".svg") {
            score -= 2;
        }
        score
    });
    candidates.into_iter().next()
}

fn collect_icon_files(directory: &Path, result: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_icon_files(&path, result);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("png" | "svg" | "xpm")
        ) {
            result.push(path);
        }
    }
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

/// Parse a Debian control paragraph, including continuation lines.
pub fn parse_control_fields(control: &str) -> BTreeMap<String, String> {
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Option<String> = None;

    for line in control.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(name) = current.as_ref()
                && let Some(value) = fields.get_mut(name)
            {
                value.push('\n');
                value.push_str(line.trim_start());
            }
            continue;
        }

        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_owned();
        if name.is_empty() {
            current = None;
            continue;
        }
        fields.insert(name.clone(), value.trim_start().to_owned());
        current = Some(name);
    }

    fields
}

/// Debian descriptions use a single-line short description followed by
/// indented paragraphs.  Turn the latter into readable GTK text without
/// discarding blank paragraphs.
pub fn format_description(description: &str) -> String {
    let mut lines = description.lines();
    let Some(summary) = lines.next() else {
        return String::new();
    };

    let mut result = String::from(summary.trim());
    for line in lines {
        let line = line.trim_end();
        if line.trim() == "." {
            result.push('\n');
        } else {
            result.push('\n');
            result.push_str(line.trim_start());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{format_description, parse_control_fields};

    #[test]
    fn parses_continuation_lines() {
        let fields = parse_control_fields(
            "Package: demo\nVersion: 1.2\nDescription: A short summary\n A long line\n .\n another paragraph\n",
        );
        assert_eq!(fields["Package"], "demo");
        assert_eq!(
            fields["Description"],
            "A short summary\nA long line\n.\nanother paragraph"
        );
    }

    #[test]
    fn formats_debian_paragraphs() {
        assert_eq!(
            format_description("Summary\n details\n .\n next"),
            "Summary\ndetails\n\nnext"
        );
    }
}
