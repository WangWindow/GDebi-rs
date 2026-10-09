# GDebi-rs

Lightweight GTK4 Debian package installer for inspecting and installing local
`.deb` packages.

## Upstream

The original GDebi source is used as a behavioral reference:

<https://codegraph.jelmer.uk/gdebi/0.9.5.8/GDebi>

## Dependencies

Build dependencies:

- Rust 1.70+
- GTK 4.0+
- gettext

Runtime dependencies:

- `dpkg-deb`
- `apt-get`
- `pkexec`

On Debian/Ubuntu:

```sh
sudo apt install libcairo2-dev libgtk-4-dev gettext \
    dpkg apt policykit-1
```

## Build and run

```sh
cargo build --release
./target/release/gdebi-rs package.deb
```

Command-line is also available:

```sh
./target/release/gdebi-rs --cli package.deb
```

## Source layout

| Module | Purpose |
| --- | --- |
| `src/gdebi/deb_package.rs` | Debian package parsing |
| `src/gdebi/gdebi_common.rs` | Shared package operations |
| `src/gdebi/gdebi_gtk.rs` | GTK4 interface |
| `src/gdebi/gdebi_cli.rs` | CLI frontend |
| `src/installer.rs` | APT/polkit installation |
| `src/i18n.rs`, `po/` | gettext translations |

## License

GPL-2.0-or-later.
