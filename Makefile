.PHONY: build test check install uninstall

build:
	cargo build --release --locked

test:
	cargo test --locked --no-default-features

check:
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	desktop-file-validate data/io.github.joeytong.cosmic-applet-disk-space.desktop

install:
	sh scripts/install.sh

uninstall:
	sh scripts/uninstall.sh
