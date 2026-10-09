//! Small command-line frontend, kept separate from the GTK frontend like
//! the upstream `GDebiCli.py` module.

use super::gdebi_common::GDebiCommon;
use crate::i18n::tr;
use crate::installer::{self, InstallEvent};
use std::path::Path;
use std::sync::mpsc;

pub struct GDebiCli;

impl GDebiCli {
    pub fn run(path: impl AsRef<Path>) -> i32 {
        let package = match GDebiCommon::open(path) {
            Ok(package) => package,
            Err(error) => {
                eprintln!("gdebi: {error}");
                return 1;
            }
        };

        println!(
            "{} {} ({})",
            package.package_name(),
            package.version(),
            package.architecture()
        );
        println!("{}", GDebiCommon::dependency_summary(&package));
        println!("{}", tr("Installing…"));

        let (sender, receiver) = mpsc::channel();
        installer::install(&package.path, sender);
        loop {
            match receiver.recv() {
                Ok(InstallEvent::Output(line)) => println!("{line}"),
                Ok(InstallEvent::Finished(result)) => match result {
                    Ok(()) => return 0,
                    Err(error) => {
                        eprintln!("gdebi: {error}");
                        return 1;
                    }
                },
                Err(_) => {
                    eprintln!(
                        "gdebi: {}",
                        tr("The installation process exited unexpectedly")
                    );
                    return 1;
                }
            }
        }
    }
}
