{
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "tmux-sessionizer";
  version = "0.1.0";

  src = ./.;

  # cargoHash = "sha256-6tHRl/InIqjX76zX970jv4rBC31BQm93d52u87LyZwI=";
  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  meta = {
    description = "";
    homepage = "https://https://github.com/SoxPopuli/tmux-sessionizer";
  };
})
