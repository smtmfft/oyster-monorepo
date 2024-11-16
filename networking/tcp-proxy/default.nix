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
in rec {
  default = naersk'.buildPackage {
    src = ./.;
    CARGO_BUILD_TARGET = target;
    TARGET_CC = "${pkgs.pkgsStatic.stdenv.cc}/bin/cc";
    nativeBuildInputs = [ pkgs.pkgsStatic.stdenv.cc ];
  };
  compressed = pkgs.runCommand "compressed" {
    nativeBuildInputs = [ pkgs.upx ];
  } ''
    mkdir -p $out/bin
    cp ${default}/bin/* $out/bin/
    chmod +w $out/bin/*
    upx $out/bin/*
  '';
}
