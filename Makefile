fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy -- -D warnings
