set dotenv-load := true

default: check

check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features

format:
    cargo fmt --all

run *args:
    cargo run -- {{args}}

