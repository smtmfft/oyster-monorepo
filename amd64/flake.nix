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
		supervisord = pkgs.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/supervisord_c2cae38b_linux_amd64";
			sha256 = "46bf15be56a4cac3787f3118d5b657187ee3e4d0a36f3aa2970f3ad3bd9f2712";
		};
		in {
			app = pkgs.runCommand "app" {} ''
			echo Preparing the app folder
			mkdir -p $out
			mkdir -p $out/app
			cp ${supervisord} $out/app/supervisord
			'';
			packages.${system}.default = nitro.buildEif {
				name = "enclave";
				arch = eifArch;

				# use AWS' nitro-cli binary blobs
				inherit (nitro.blobs.${eifArch}) kernel kernelConfig nsmKo;

				entrypoint = "/bin/hello";
				env = "";
				copyToRoot = pkgs.buildEnv {
					name = "image-root";
					paths = [ pkgs.hello self.app ];
					pathsToLink = [ "/bin" "/app" ];
				};
			};
		};
}
