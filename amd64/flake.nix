{
	inputs = {
		nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
		nitro-util.url = "github:/monzo/aws-nitro-util";
		nitro-util.inputs.nixpkgs.follows = "nixpkgs";
	};
	outputs = { self, nixpkgs, flake-utils, nitro-util }:
		let system = "x86_64-linux"; 
		nitro = nitro-util.lib.${system};
		eifArch = "x86_64";
		pkgs = nixpkgs.legacyPackages."${system}";
		in {
			packages.${system}.default = nitro.buildEif {
				name = "enclave.eif";
				arch = eifArch;

				# use AWS' nitro-cli binary blobs
				inherit (nitro.blobs.${eifArch}) kernel kernelConfig nsmKo;

				entrypoint = "/bin/hello";
				env = "";
				copyToRoot = pkgs.buildEnv {
					name = "image-root";
					paths = [ pkgs.hello ];
					pathsToLink = [ "/bin" ];
				};
			};
		};
}
