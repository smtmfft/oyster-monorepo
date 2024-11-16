{
  nixpkgs,
  systemConfig,
  fenix,
  naersk,
}: let
  system = systemConfig.system;
  pkgs = nixpkgs.legacyPackages."${system}";
  target = systemConfig.rust_target;
  toolchain = with fenix.packages.${system};
    combine [
      stable.cargo
      stable.rustc
      targets.${target}.stable.rust-std
    ];
  naersk' = naersk.lib.${system}.override {
    cargo = toolchain;
    rustc = toolchain;
  };
in {
  default = naersk'.buildPackage {
    src = ./.;
    CARGO_BUILD_TARGET = target;
    TARGET_CC = "${gccPkgs.gcc}/bin/cc";
    nativeBuildInputs = [ pkgs.pkgsStatic.stdenv.cc ];
  };
}
