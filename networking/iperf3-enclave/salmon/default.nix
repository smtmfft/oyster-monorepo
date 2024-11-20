{
  nixpkgs,
  systemConfig,
  nitro-util,
  supervisord,
  dnsproxy,
  keygen,
  tcp-proxy,
  attestation-server,
  kernels,
}: let
  system = systemConfig.system;
  nitro = nitro-util.lib.${system};
  eifArch = systemConfig.eif_arch;
  pkgs = nixpkgs.legacyPackages."${system}";
  supervisord' = "${supervisord}/bin/supervisord";
  dnsproxy' = "${dnsproxy}/bin/dnsproxy";
  keygenEd25519 = "${keygen}/bin/keygen-ed25519";
  itvtProxy = "${tcp-proxy}/bin/ip-to-vsock-transparent";
  vtiProxy = "${tcp-proxy}/bin/vsock-to-ip";
  attestationServer = "${attestation-server}/bin/oyster-attestation-server";
  kernel = kernels.kernel;
  kernelConfig = kernels.kernelConfig;
  nsmKo = kernels.nsmKo;
  init = kernels.init;
  setup = ./. + "/setup.sh";
  supervisorConf = ./. + "/supervisord.conf";
  app = pkgs.runCommand "app" {} ''
    echo Preparing the app folder
    pwd
    mkdir -p $out
    mkdir -p $out/app
    mkdir -p $out/etc
    cp ${supervisord'} $out/app/supervisord
    cp ${keygenEd25519} $out/app/keygen-ed25519
    cp ${itvtProxy} $out/app/ip-to-vsock-transparent
    cp ${vtiProxy} $out/app/vsock-to-ip
    cp ${attestationServer} $out/app/attestation-server
    cp ${dnsproxy'} $out/app/dnsproxy
    cp ${setup} $out/app/setup.sh
    chmod +x $out/app/*
    cp ${supervisorConf} $out/etc/supervisord.conf
  '';
  # kinda hacky, my nix-fu is not great, figure out a better way
  initPerms = pkgs.runCommand "initPerms" {} ''
    cp ${init} $out
    chmod +x $out
  '';
in {
  default = nitro.buildEif {
    name = "enclave";
    arch = eifArch;

    init = initPerms;
    kernel = kernel;
    kernelConfig = kernelConfig;
    nsmKo = nsmKo;
    cmdline = builtins.readFile nitro.blobs.${eifArch}.cmdLine;

    entrypoint = "/app/setup.sh";
    env = "";
    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [app pkgs.busybox pkgs.nettools pkgs.iproute2 pkgs.iptables-legacy pkgs.iperf3];
      pathsToLink = ["/bin" "/app" "/etc"];
    };
  };
}
