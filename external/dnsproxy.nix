{
  nixpkgs,
  systemConfig,
}: let
  system = systemConfig.system;
  pkgs = nixpkgs.legacyPackages."${system}";
  src = pkgs.fetchFromGitHub {
    owner = "AdguardTeam";
    repo = "dnsproxy";
    rev = "v0.73.2";
    sha256 = "sha256-Xxi23Cwm389fsDcYa3qJ9GhDZVXwh/LiWPfiYMuG5Js=";
  };
in rec {
  # static by default since CGO is disabled
  uncompressed = pkgs.buildGoModule {
    src = src;
    name = "dnsproxy";
    vendorHash = "sha256-tyEp0vY8hWE8jTvkxKuqQJcgeey+c50pxREpmlZWE24=";
    ldflags = ["-s" "-w"];
    trimpath = true;
    buildMode = "pie";
    tags = ["netgo" "osusergo"];
    subPackages = [ "." ];
  };

  compressed =
    pkgs.runCommand "compressed" {
      nativeBuildInputs = [pkgs.upx];
    } ''
      mkdir -p $out/bin
      cp ${uncompressed}/bin/* $out/bin/
      chmod +w $out/bin/*
      upx $out/bin/*
    '';

  default = compressed;
}
