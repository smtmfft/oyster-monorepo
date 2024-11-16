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
  in {
    formatter =
      systems.forSystems systems.systems (systemConfig: nixpkgs.legacyPackages.${systemConfig.system}.alejandra);
    packages = systems.forSystems systems.systems (systemConfig: {
      attestation-server = import ./attestation/server {
        inherit nixpkgs systemConfig fenix naersk;
      };
    });
  };
}
