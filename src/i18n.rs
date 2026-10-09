//! gettext setup shared by both frontends.

use gettextrs::{TextDomain, gettext};

pub const DOMAIN: &str = "gdebi-rs";

pub fn init() {
    // English is the msgid language. If no catalog matches the current
    // locale, gettext naturally returns the original English msgid.
    let _ = TextDomain::new(DOMAIN)
        .prepend(env!("OUT_DIR"))
        .push("/usr/local/share")
        .push("/usr/share")
        .init();
}

pub fn tr(message: &str) -> String {
    gettext(message)
}
