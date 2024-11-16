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
  gccPkgs = if systemConfig.musl then pkgs.pkgsMusl else pkgs;
in rec {
  default = naersk'.buildPackage {
    src = ./.;
    CARGO_BUILD_TARGET = target;
    TARGET_CC = "${gccPkgs.gcc}/bin/cc";
    buildInputs = [
      gccPkgs.gcc
    ];
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
