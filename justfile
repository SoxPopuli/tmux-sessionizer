alias br := build-release
alias tr := test-release

install_dir := "~/.config/tools/bin"
bin_name := "tmux-sessionizer"

[private]
list:
    @ just -l

build-release:
    cargo build --release

test-release: build-release
    cargo test --release

install: test-release
    cp -vf {{ "./target/release" / bin_name }} {{ install_dir / "tmux-sessionizer" }}

clean-release:
    cargo clean -r
