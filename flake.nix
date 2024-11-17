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
    systemBuilder = systemConfig: {
      attestation.server = import ./attestation/server {
        inherit nixpkgs systemConfig fenix naersk;
      };
      attestation.verifier = import ./attestation/verifier {
        inherit nixpkgs systemConfig fenix naersk;
      };
      initialization.keygen = import ./initialization/keygen {
        inherit nixpkgs systemConfig fenix naersk;
      };
      initialization.vet = import ./initialization/vet {
        inherit nixpkgs systemConfig fenix naersk;
      };
      networking.raw-proxy = import ./networking/raw-proxy {
        inherit nixpkgs systemConfig fenix naersk;
      };
      networking.tcp-proxy = import ./networking/tcp-proxy {
        inherit nixpkgs systemConfig fenix naersk;
      };
    };
  in {
    formatter = {
      "x86_64-linux" = nixpkgs.legacyPackages."x86_64-linux".alejandra;
      "aarch64-linux" = nixpkgs.legacyPackages."aarch64-linux".alejandra;
    };
    packages = {
      "x86_64-linux" = rec {
        gnu = systemBuilder {
          system = "x86_64-linux";
          rust_target = "x86_64-unknown-linux-gnu";
          eif_arch = "x86_64";
          static = false;
        };
        musl = systemBuilder {
          system = "x86_64-linux";
          rust_target = "x86_64-unknown-linux-musl";
          eif_arch = "x86_64";
          static = true;
        };
        default = musl;
      };
      "aarch64-linux" = rec {
        gnu = systemBuilder {
          system = "aarch64-linux";
          rust_target = "aarch64-unknown-linux-gnu";
          eif_arch = "aarch64";
          static = false;
        };
        musl = systemBuilder {
          system = "aarch64-linux";
          rust_target = "aarch64-unknown-linux-musl";
          eif_arch = "aarch64";
          static = true;
        };
        default = musl;
      };
    };
  };
}
