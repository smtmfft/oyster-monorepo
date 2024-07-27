{
	inputs = {
		# latest commit on 23.11 channel as of 17th July 2024
		nixpkgs.url = "github:NixOS/nixpkgs/205fd4226592cc83fd4c0885a3e4c9c400efabb5";
		# latest commit on master branch as of 17th July 2024
		nitro-util.url = "github:/monzo/aws-nitro-util/7591f28388e531c5fbb7a8fc9f9d2bc3b5c05894";
		nitro-util.inputs.nixpkgs.follows = "nixpkgs";
	};
	outputs = { self, nixpkgs, nitro-util }:
		let system = "aarch64-linux"; 
		nitro = nitro-util.lib.${system};
		eifArch = "aarch64";
		pkgs = nixpkgs.legacyPackages."${system}";
		supervisord = pkgs.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/supervisord_c2cae38b_linux_arm64";
			sha256 = "7abe45b4c83389a2d7aa5879d704494aced703bfa750e3954a4ea97c9a0ea04d";
		};
		keygenEd25519 = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/keygen-ed25519_v1.0.0_linux_arm64";
			sha256 = "9073cb46950c392bba4f0439ba836bce09039cb0a2bf59cd2009fe7593d1415f";
		};
		itvtProxy = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/ip-to-vsock-transparent_v1.0.0_linux_arm64";
			sha256 = "4a1beedb1a956e350ab38d52d3bfb557aff37562a10c7f42ca394c0e2f574a7e";
		};
		vtiProxy = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/vsock-to-ip_v1.0.0_linux_arm64";
			sha256 = "c55bd946a100f8e49b75c46e2e5d4bbb6be134e2f35b0d0927afeeca55fba5d0";
		};
		attestationServer = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/attestation-server_v2.0.0_linux_arm64";
			sha256 = "4be991730c3665ebd3d5a49f9514c34da9f4d2624ca15ee54b76258f8623cf49";
		};
		dnsproxy = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/dnsproxy_v0.72.0_linux_arm64";
			sha256 = "f1a9efa733c412760f596b1ca480ed53c45f0c5a1ca251d98be277e9087d004e";
		};
		keygenSecp256k1 = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/keygen-secp256k1_v1.0.0_linux_arm64";
			sha256 = "cbb170eff52f0938aab9dd85f7174f5e7d7858e3b2be8a179f188f64cff4d4e7";
		};
		attestationVerifier = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/attestation-verifier_v2.1.0_linux_arm64";
			sha256 = "1fb9424b8972bedc80ee2bafa2d662e8dde01f9615542beec30497a37da19cb1";
		};
		kernel = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/vanilla_7614f199_arm64/Image";
			sha256 = "62d5c3f1217c2d488acef23a190d6c01080c47662596325838da7889c806c8a3";
		};
		kernelConfig = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/vanilla_7614f199_arm64/Image.config";
			sha256 = "c147022ad53d564ef6774be918f654ef60d9333b0a307a4e521879dd959941b4";
		};
		nsmKo = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/vanilla_7614f199_arm64/nsm.ko";
			sha256 = "2535b8b4e0b8697c33ba3bb64e3ca15360ceea75eb6cfba6ca3d86300c250368";
		};
		init = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/vanilla_7614f199_arm64/init";
			sha256 = "1d02f2fabd6574903c624bce6499fbd9d92283afefe6f9e5c2eead6917463c53";
		};
		setup = ./. + "/../setup.sh";
		supervisorConf = ./. + "/../supervisord.conf";
		in {
			app = pkgs.runCommand "app" {} ''
			echo Preparing the app folder
			pwd
			mkdir -p $out
			mkdir -p $out/app
			mkdir -p $out/etc
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
			cp ${supervisorConf} $out/etc/supervisord.conf
			'';
			packages.${system}.default = nitro.buildEif {
				name = "enclave";
				arch = eifArch;

				inherit (nitro.blobs.${eifArch}) init;
				kernel = kernel;
				kernelConfig = kernelConfig;
				nsmKo = nsmKo;

				entrypoint = "/app/setup.sh";
				env = "";
				copyToRoot = pkgs.buildEnv {
					name = "image-root";
					paths = [ self.app pkgs.busybox pkgs.nettools pkgs.iproute2 pkgs.iptables-legacy ];
					pathsToLink = [ "/bin" "/app" "/etc" ];
				};
			};
		};
}
