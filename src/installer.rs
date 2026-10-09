//! Privileged installation without restarting the graphical application.
//!
//! The old GDebi GTK client used `pkexec` to start a second, root-owned GTK
//! process.  On recent polkit versions that process can lose DISPLAY and
//! XAUTHORITY, which explains the `GtkStyleContext without a display
//! connection` crash after the password dialog.  This module only elevates
//! the non-graphical `apt-get` child.  The original GTK process remains in the
//! user's session for the whole operation.

use crate::i18n::tr;

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

#[derive(Debug)]
pub enum InstallEvent {
    Output(String),
    Finished(Result<(), String>),
}

pub fn install(path: &Path, events: Sender<InstallEvent>) {
    let path = path.to_owned();
    thread::spawn(move || {
        let result = run_install(&path, &events);
        let _ = events.send(InstallEvent::Finished(result));
    });
}

fn run_install(path: &Path, events: &Sender<InstallEvent>) -> Result<(), String> {
    let apt = if is_root() { "apt-get" } else { "pkexec" };

    let mut command = Command::new(apt);
    if apt == "pkexec" {
        // pkexec intentionally sanitizes the environment.  Run apt through
        // env so debconf cannot unexpectedly try to open a terminal while
        // the graphical window remains in the user's session.
        command.args([
            "/usr/bin/env",
            "DEBIAN_FRONTEND=noninteractive",
            "APT_LISTCHANGES_FRONTEND=none",
            "/usr/bin/apt-get",
        ]);
    }

    // apt-get understands an absolute .deb path and resolves its dependencies.
    // Do not use a shell here: a filename can contain spaces or shell syntax.
    command.args([
        "--yes",
        "--option=Dpkg::Options::=--force-confold",
        "install",
    ]);
    command.arg(path);
    command
        .env("DEBIAN_FRONTEND", "noninteractive")
        .env("APT_LISTCHANGES_FRONTEND", "none")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|error| {
        if apt == "pkexec" {
            format!(
                "{}: {error}",
                tr("Unable to start the polkit helper pkexec")
            )
        } else {
            format!("{}: {error}", tr("Unable to start apt-get"))
        }
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| tr("Unable to read apt-get standard output"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| tr("Unable to read apt-get error output"))?;

    let stdout_events = events.clone();
    let stdout_thread = thread::spawn(move || forward_lines(stdout, stdout_events));
    let stderr_events = events.clone();
    let stderr_thread = thread::spawn(move || forward_lines(stderr, stderr_events));

    let status = child
        .wait()
        .map_err(|error| format!("{}: {error}", tr("Waiting for apt-get failed")))?;
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    if status.success() {
        Ok(())
    } else if let Some(code) = status.code() {
        Err(format!(
            "{}: {code}",
            tr("apt-get installation failed with exit status")
        ))
    } else {
        Err(tr(
            "apt-get was terminated by a signal; installation did not finish",
        ))
    }
}

fn forward_lines<R: std::io::Read + Send + 'static>(reader: R, events: Sender<InstallEvent>) {
    for line in BufReader::new(reader).lines() {
        match line {
            Ok(line) if !line.is_empty() => {
                let _ = events.send(InstallEvent::Output(line));
            }
            Ok(_) => {}
            Err(error) => {
                let _ = events.send(InstallEvent::Output(format!(
                    "{}: {error}",
                    tr("Failed to read installation output")
                )));
                break;
            }
        }
    }
}

#[cfg(unix)]
fn is_root() -> bool {
    // This is only a decision about whether pkexec is needed; apt still
    // performs all of its own authorization and lock checks.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn is_root() -> bool {
    false
}
