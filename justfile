alias br := build-release
alias tr := test-release

build-release:
    cargo build --release

test-release: build-release
    cargo test --release
