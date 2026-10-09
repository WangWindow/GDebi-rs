# GDebi-rs

Lightweight GTK4 Debian package installer for local `.deb` files.

## Dependencies

- Rust stable
- GTK4 4.0+
- gettext
- Runtime: `dpkg-deb`, `apt-get`
- Recommended for graphical authorization: `pkexec` or `policykit-1`

Debian/Ubuntu build dependencies:

```sh
sudo apt install libcairo2-dev libgtk-4-dev gettext dpkg-dev
```

## Build and run

```sh
cargo build --release
./target/release/gdebi-rs package.deb
```

CLI mode:

```sh
./target/release/gdebi-rs --cli package.deb
```

## Release packages

Pushing a `v*` tag starts GitHub Actions and publishes Debian packages for:

- `amd64`
- `armhf`
- `arm64`

## Reference

The original GDebi source used for behavioral reference:

<https://codegraph.jelmer.uk/gdebi/0.9.5.8/GDebi>

## License

GPL-2.0-or-later.
