.PHONY: fmt lint test check release package checksums completions audit

fmt:
	cargo fmt --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

check: fmt lint test

release:
	cargo build --release

package:
	cargo package

checksums: release
	shasum -a 256 target/release/minecli > target/release/minecli.sha256

completions:
	mkdir -p target/completions
	cargo run -- completions bash > target/completions/minecli.bash
	cargo run -- completions zsh > target/completions/_minecli
	cargo run -- completions fish > target/completions/minecli.fish

audit:
	@if command -v cargo-audit >/dev/null 2>&1; then \
		cargo audit; \
	else \
		echo "cargo-audit is not installed. Install with: cargo install cargo-audit"; \
		exit 1; \
	fi
