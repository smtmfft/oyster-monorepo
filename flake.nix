{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = {
    self,
    nixpkgs,
    fenix,
    naersk,
  }: let
    systems = import ./systems.nix;
    systemBuilder = (systemConfig: {
      attestation.server = import ./attestation/server {
        inherit nixpkgs systemConfig fenix naersk;
      };
      networking.tcp-proxy = import ./networking/tcp-proxy {
        inherit nixpkgs systemConfig fenix naersk;
      };
    });
  in {
    formatter =
      systems.forSystems systems.systems (systemConfig: nixpkgs.legacyPackages.${systemConfig.system}.alejandra);
    packages = {
      "x86_64-linux" = rec {
        gnu = systemBuilder {
          system = "x86_64-linux";
          rust_target = "x86_64-unknown-linux-gnu";
          static = false;
        };
        musl = systemBuilder {
          system = "x86_64-linux";
          rust_target = "x86_64-unknown-linux-musl";
          static = true;
        };
        default = musl;
      };
      "aarch64-linux" = rec {
        gnu = systemBuilder {
          system = "aarch64-linux";
          rust_target = "aarch64-unknown-linux-gnu";
          static = false;
        };
        musl = systemBuilder {
          system = "aarch64-linux";
          rust_target = "aarch64-unknown-linux-musl";
          static = true;
        };
        default = musl;
      };
    };
  };
}
