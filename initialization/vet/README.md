![Marlin Oyster Logo](./logo.svg)

# Vet

Curl-like utility to make http requests over vsocks.

## Build

```bash
cargo build --release
```

### Reproducible builds

Reproducible builds can be done using Nix. The monorepo provides a Nix flake which includes this project and can be used to trigger builds:

```bash
nix build -v .#<flavor>.initialization.vet.<output>
```

Supported flavors:
- `gnu`
- `musl`

Supported outputs:
- `default`, same as `compressed`
- `uncompressed`
- `compressed`, using `upx`

## Prebuilt binaries

amd64: https://artifacts.marlin.org/oyster/binaries/vet_v1.0.0_linux_amd64 \
checksum: cc232f2bbf4a808638ddf54ed19e79ebfcba558a7fb902c02d7a8f92562231a9

arm64: https://artifacts.marlin.org/oyster/binaries/vet_v1.0.0_linux_arm64 \
checksum: f052d9f257caf5212c9b65e8c7cd44bfd00fe38f2596cc7a9b6d8f06ecfeff4a

## Usage

```bash
$ ./target/release/vet --help
Usage: vet --url <URL>

Options:
  -u, --url <URL>  url to query
  -h, --help       Print help
  -V, --version    Print version
```

## Example

```
$ vet --url 3:1500/oyster/job
0x1234567812345678123456781234567812345678123456781234567812345678
```
