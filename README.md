![Marlin Oyster Logo](./logo.svg)

# Attestation Verifier Enclave

The attestation verifier enclave packages the [attestation verifier](https://github.com/marlinprotocol/oyster-attestation-verifier) along with related services in an enclave.

## Prerequisites

- Nix
- (only if cross compiling) A [binfmt emulator](https://github.com/tonistiigi/binfmt) for the target platform

The attestation verifier enclave is built using nix for reproducibility. It does NOT use the standard `nitro-cli` based pipeline, and instead uses [monzo/aws-nitro-util](https://github.com/monzo/aws-nitro-util) in order to produce bit-for-bit reproducible enclaves.

The following nix `experimental-features` must be enabled:
- nix-command
- flakes

## Build

```bash
# On amd64, For amd64
# The request-amd64 folder will contain the enclave image and pcrs
nix build --out-link result-amd64 ./amd64 -v -L

# On arm64, For arm64
# The request-amd64 folder will contain the image and pcrs
nix build --out-link result-arm64 ./arm64 -v -L
```

## Cross builds

Cross builds do work, but can potentially take a really long time due to the use of qemu to emulate the target platform (can be a few hours). It should produce bit-for-bit identical enclave images compared to native builds.

```bash
# On arm64, For amd64
# The request-amd64 folder will contain the enclave image and pcrs
nix build --out-link result-amd64 ./amd64 -v -L --system x86_64-linux

# On amd64, For arm64
# The request-amd64 folder will contain the image and pcrs
nix build --out-link result-arm64 ./arm64 -v -L --system aarch64-linux
```

## Prebuilt enclaves

amd64: https://artifacts.marlin.org/oyster/eifs/attestation-verifier_v1.1.0_linux_amd64.eif \
checksum: f50bd44512132aa7a8bae6e73c8a1c2120b76061707dcc60a2196d7a3d17bb45 \
pcrs: https://artifacts.marlin.org/oyster/eifs/attestation-verifier_v1.1.0_linux_amd64.json

arm64: https://artifacts.marlin.org/oyster/eifs/attestation-verifier_v1.1.0_linux_arm64.eif \
checksum: f390fe6a75f3fea548ae806871c2b2d64bea24bc17a445cda019dc6953c5f01f \
pcrs: https://artifacts.marlin.org/oyster/eifs/attestation-verifier_v1.1.0_linux_arm64.json
