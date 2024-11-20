{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/24.05";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nitro-util = {
      url = "github:monzo/aws-nitro-util";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = {
    self,
    nixpkgs,
    fenix,
    naersk,
    nitro-util,
  }: let
    systemBuilder = systemConfig: rec {
      external.dnsproxy = import ./external/dnsproxy.nix {
        inherit nixpkgs systemConfig;
      };
      external.supervisord = import ./external/supervisord.nix {
        inherit nixpkgs systemConfig;
      };
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
      kernels.vanilla = import ./kernels/vanilla.nix {
        inherit nixpkgs systemConfig;
      };
      kernels.tuna = import ./kernels/tuna.nix {
        inherit nixpkgs systemConfig;
      };
      networking.raw-proxy = import ./networking/raw-proxy {
        inherit nixpkgs systemConfig fenix naersk;
      };
      networking.tcp-proxy = import ./networking/tcp-proxy {
        inherit nixpkgs systemConfig fenix naersk;
      };
      networking.iperf3-enclave.salmon = import ./networking/iperf3-enclave/salmon {
        inherit nixpkgs systemConfig nitro-util;
        supervisord = external.supervisord.compressed;
        dnsproxy = external.dnsproxy.compressed;
        keygen = initialization.keygen.compressed;
        tcp-proxy = networking.tcp-proxy.compressed;
        attestation-server = attestation.server.compressed;
        kernels = kernels.vanilla;
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
