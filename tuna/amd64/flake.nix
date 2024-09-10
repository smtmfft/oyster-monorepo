{
	inputs = {
		nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
		nitro-util.url = "github:/monzo/aws-nitro-util";
		nitro-util.inputs.nixpkgs.follows = "nixpkgs";
	};
	outputs = { self, nixpkgs, nitro-util }:
		let system = "x86_64-linux";
		nitro = nitro-util.lib.${system};
		eifArch = "x86_64";
		pkgs = nixpkgs.legacyPackages."${system}";
		supervisord = pkgs.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/supervisord_c2cae38b_linux_amd64";
			sha256 = "46bf15be56a4cac3787f3118d5b657187ee3e4d0a36f3aa2970f3ad3bd9f2712";
		};
		keygenEd25519 = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/keygen-ed25519_v1.0.0_linux_amd64";
			sha256 = "e68c55cab8ff21de5b9c9ab831b3365717cceddf5f0ad82fee57d1ef40231d3c";
		};
		itvroProxy = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/ip-to-vsock-raw-outgoing_v1.0.0_linux_amd64";
			sha256 = "e94c516dd9608fe2eb2d6d6ff0be54a8f25de4cacdb289999d07bffa75364afe";
		};
		vtiriProxy = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/vsock-to-ip-raw-incoming_v1.0.0_linux_amd64";
			sha256 = "5bd7433956269cea0c92ca64b1e6abe5f763a3cad9c1011885a944cbc0ec53ee";
		};
		attestationServer = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/attestation-server_v2.0.0_linux_amd64";
			sha256 = "b05852fa4ebda4d9a88ab2b61deae5f22b7026f4d99c5eeeca3c31ee99a77a71";
		};
		dnsproxy = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/dnsproxy_v0.72.0_linux_amd64";
			sha256 = "1c2bc5eab0dcdbac89c0ef6515e328227de9987af618a7138cc05d9bc53590c1";
		};
		vet = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/binaries/vet_v1.0.0_linux_amd64";
			sha256 = "cc232f2bbf4a808638ddf54ed19e79ebfcba558a7fb902c02d7a8f92562231a9";
		};
		kernel = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/tuna_8fda9baa_amd64/bzImage";
			sha256 = "751ba1f2bdd2c2c3085d81bec544fc2bccb99da03ee1af0cbef557c03599f231";
		};
		kernelConfig = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/tuna_8fda9baa_amd64/bzImage.config";
			sha256 = "295ac5cd0027f879b501194bc40e1240d30515b48239d7fbeefe7ddae35896a6";
		};
		nsmKo = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/tuna_8fda9baa_amd64/nsm.ko";
			sha256 = "42b49249abe01a1d32639bf1011e62418ac10b0360328138ea36271451c3a587";
		};
		init = builtins.fetchurl {
			url = "https://artifacts.marlin.org/oyster/kernels/tuna_8fda9baa_amd64/init";
			sha256 = "847bac1648acedc01a76f0e0108d3f08df956ed267622a51066fd9e1d8a29ee8";
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
			cp ${itvroProxy} $out/app/ip-to-vsock-raw-outgoing
			cp ${vtiriProxy} $out/app/vsock-to-ip-raw-incoming
			cp ${attestationServer} $out/app/attestation-server
			cp ${dnsproxy} $out/app/dnsproxy
			cp ${vet} $out/app/vet
			cp ${setup} $out/app/setup.sh
			chmod +x $out/app/*
			cp ${supervisorConf} $out/etc/supervisord.conf
			'';
			# kinda hacky, my nix-fu is not great, figure out a better way
			initPerms = pkgs.runCommand "initPerms" {} ''
			cp ${init} $out
			chmod +x $out
			'';
			packages.${system}.default = nitro.buildEif {
				name = "enclave";
				arch = eifArch;

				init = self.initPerms;
				kernel = kernel;
				kernelConfig = kernelConfig;
				nsmKo = nsmKo;
				cmdline = builtins.readFile nitro.blobs.${eifArch}.cmdLine;

				entrypoint = "/app/setup.sh";
				env = "";
				copyToRoot = pkgs.buildEnv {
					name = "image-root";
					paths = [ self.app pkgs.busybox pkgs.nettools pkgs.iproute2 pkgs.iptables-legacy pkgs.ipset pkgs.iperf3 ];
					pathsToLink = [ "/bin" "/app" "/etc" ];
				};
			};
		};
}
