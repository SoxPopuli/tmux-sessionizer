alias br := build-release
alias tr := test-release

[private]
list:
    @ just -l

build-release:
    cargo build --release

test-release: build-release
    cargo test --release

install: test-release
    cp -vf ./target/release/tmux-sessionizer ~/.config/tmux-sessionizer

clean-release:
    cargo clean -r
