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
		keygenEd25519 = builtins.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/keygen-ed25519_v1.0.0_linux_amd64";
			sha256 = "e68c55cab8ff21de5b9c9ab831b3365717cceddf5f0ad82fee57d1ef40231d3c";
		};
		itvtProxy = builtins.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/ip-to-vsock-transparent_v1.0.0_linux_amd64";
			sha256 = "15ecdf4ed7c0a3f65ebfa2fb10f0c1cb60e67677162db8cca6915aabb5afd4b9";
		};
		vtiProxy = builtins.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip_v1.0.0_linux_amd64";
			sha256 = "8ad67e28b18a742c3b94078954021215b57a287ee634f09556efabcac0b99597";
		};
		attestationServer = builtins.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/attestation-server_v2.0.0_linux_amd64";
			sha256 = "b05852fa4ebda4d9a88ab2b61deae5f22b7026f4d99c5eeeca3c31ee99a77a71";
		};
		dnsproxy = builtins.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/dnsproxy_v0.72.0_linux_amd64";
			sha256 = "1c2bc5eab0dcdbac89c0ef6515e328227de9987af618a7138cc05d9bc53590c1";
		};
		keygenSecp256k1 = builtins.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/keygen-secp256k1_v1.0.0_linux_amd64";
			sha256 = "9d4344e491413abb559e507ccfcd4397edf736199fb1a1a39c9ae9c576655579";
		};
		attestationVerifier = builtins.fetchurl {
			url = "http://public.artifacts.marlin.pro/projects/enclaves/attestation-verifier_v2.1.0_linux_amd64";
			sha256 = "6f32346254fefef7934d965f6341ea340f2a06bf183fb1c8053d7b53f00e097d";
		};
		setup = ./. + "/../setup.sh";
		supervisorConf = ./. + "/../supervisord.conf";
		in {
			app = pkgs.runCommand "app" {} ''
			echo Preparing the app folder
			pwd
			mkdir -p $out
			mkdir -p $out/app
			cp ${supervisord} $out/app/supervisord
			cp ${keygenEd25519} $out/app/keygen-ed25519
			cp ${itvtProxy} $out/app/ip-to-vsock-transparent
			cp ${vtiProxy} $out/app/vsock-to-ip
			cp ${attestationServer} $out/app/attestation-server
			cp ${dnsproxy} $out/app/dnsproxy
			cp ${keygenSecp256k1} $out/app/keygen-secp256k1
			cp ${attestationVerifier} $out/app/attestation-verifier
			cp ${setup} $out/app/setup.sh
			chmod +x $out/app/*
			cp ${supervisorConf} $out/app/supervisord.conf
			'';
			packages.${system}.default = nitro.buildEif {
				name = "enclave";
				arch = eifArch;

				# use AWS' nitro-cli binary blobs
				inherit (nitro.blobs.${eifArch}) kernel kernelConfig nsmKo;

				entrypoint = "/app/setup.sh";
				env = "";
				copyToRoot = pkgs.buildEnv {
					name = "image-root";
					paths = [ self.app pkgs.nettools pkgs.iproute2 pkgs.iptables-legacy ];
					pathsToLink = [ "/bin" "/app" ];
				};
			};
		};
}
