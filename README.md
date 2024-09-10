![Marlin Oyster Logo](./logo.svg)

# Iperf3 Enclave

The iperf3 enclave packages iperf3 along with related services in an enclave for benchmarking networking throughput.

## Prerequisites

- Nix
- (only if cross compiling) A [binfmt emulator](https://github.com/tonistiigi/binfmt) for the target platform

The attestation verifier enclave is built using nix for reproducibility. It does NOT use the standard `nitro-cli` based pipeline, and instead uses [monzo/aws-nitro-util](https://github.com/monzo/aws-nitro-util) in order to produce bit-for-bit reproducible enclaves.

The following nix `experimental-features` must be enabled:
- nix-command
- flakes

## Build

```bash
# Salmon family, On amd64, For amd64
# The result-salmon-amd64 folder will contain the enclave image and pcrs
nix build --out-link result-salmon-amd64 ./salmon/amd64 -v -L

# Salmon family, On arm64, For arm64
# The result-salmon-arm64 folder will contain the image and pcrs
nix build --out-link result-salmon-arm64 ./salmon/arm64 -v -L

# Tuna family, On amd64, For amd64
# The result-tuna-amd64 folder will contain the enclave image and pcrs
nix build --out-link result-tuna-amd64 ./tuna/amd64 -v -L

# Tuna family, On arm64, For arm64
# The result-tuna-arm64 folder will contain the image and pcrs
nix build --out-link result-tuna-arm64 ./tuna/arm64 -v -L
```

## Cross builds

Cross builds do work, but can potentially take a really long time due to the use of qemu to emulate the target platform (can be a few hours). It should produce bit-for-bit identical enclave images compared to native builds.

```bash
# Salmon family, On arm64, For amd64
# The result-salmon-amd64 folder will contain the enclave image and pcrs
nix build --out-link result-salmon-amd64 ./salmon/amd64 -v -L --system x86_64-linux

# Salmon family, On amd64, For arm64
# The result-salmon-arm64 folder will contain the enclave image and pcrs
nix build --out-link result-salmon-arm64 ./salmon/arm64 -v -L --system aarch64-linux

# Tuna family, On arm64, For amd64
# The result-tuna-amd64 folder will contain the image and pcrs
nix build --out-link result-tuna-amd64 ./tuna/amd64 -v -L --system x86_64-linux

# Tuna family, On amd64, For arm64
# The result-tuna-arm64 folder will contain the image and pcrs
nix build --out-link result-tuna-arm64 ./tuna/arm64 -v -L --system aarch64-linux
```

## Prebuilt enclaves

amd64: https://artifacts.marlin.org/oyster/eifs/iperf3-salmon_v1.0.0_linux_amd64.eif \
checksum: 8f33084e50bf94772d84ffcb2db4ee6d036e5476158d37bf307a8510bb6a720b \
pcrs: https://artifacts.marlin.org/oyster/eifs/iperf3-salmon_v1.0.0_linux_amd64.json

arm64: https://artifacts.marlin.org/oyster/eifs/iperf3-salmon_v1.0.0_linux_arm64.eif \
checksum: e9d112aad8f52d1a12bf362e30d069e40950db84a51eaaf3d22c7a93b6fe8d5c \
pcrs: https://artifacts.marlin.org/oyster/eifs/iperf3-salmon_v1.0.0_linux_arm64.json

amd64: https://artifacts.marlin.org/oyster/eifs/iperf3-tuna_v1.0.0_linux_amd64.eif \
checksum: 279a7422ae115d5a4ac446d7c2c5ce18e64bf42ecfe4b133892fea379b1ec66b \
pcrs: https://artifacts.marlin.org/oyster/eifs/iperf3-tuna_v1.0.0_linux_amd64.json

arm64: https://artifacts.marlin.org/oyster/eifs/iperf3-tuna_v1.0.0_linux_arm64.eif \
checksum: e08913e0df9b5e49e734b268c7238531236433e22f5ddde9ef0accd931484edc \
pcrs: https://artifacts.marlin.org/oyster/eifs/iperf3-tuna_v1.0.0_linux_arm64.json
