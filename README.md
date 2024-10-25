![Marlin Oyster Logo](./logo.svg)

# Attestation Verifier - RiscZero

This repository implements a RiscZero based AWS Nitro Enclave attestation verifier.

While it produces zero false positives, it does not aim to produce zero false negatives, i.e. it could reject _theoretically_ valid attestations. Instead, it asserts specific attestation formats that are _actually_ used in order to optimize proving time. It also does not verify any extensions in the certificates as it was deemed unnecessary.

## Build

Install the RiscZero tooling before proceeding further.

Note: Requires CUDA by default. It is possible to disable CUDA by disabling the relevant feature in `host/Cargo.toml`, but the proof generation process could take hours on a CPU. 

```bash
cargo build --release
```

### Reproducible builds

Reproducible builds are enabled for the guest to produce a consistent GUEST_ID.

Expected GUEST_ID: 0x541487c38ab4802160c0e4058c4e0d66c247ed89d373685fd7171c7836e59629

## Usage

```bash
$ ./target/release/host --help
GUEST: [84, 20, 135, 195, 138, 180, 128, 33, 96, 192, 228, 5, 140, 78, 13, 102, 194, 71, 237, 137, 211, 115, 104, 95, 215, 23, 28, 120, 54, 229, 150, 41]
Usage: host --url <URL>

Options:
  -u, --url <URL>  
  -h, --help       Print help
  -V, --version    Print version
```

It takes in a URL to an attestation server producing binary attestations. The attestation server should include a 64 byte public key in the attestation.

## Journal format

The journal contains bytes in the following order:
- 8 byte timestamp in milliseconds from the attestation
- 48 byte PCR0
- 48 byte PCR1
- 48 byte PCR2
- 48 byte public key from the root certificate
- 64 byte public key from the attestation
- 2 byte length of the user data
- N byte user data

## Directory Structure

```text
project_name
├── Cargo.toml
├── host
│   ├── Cargo.toml                     <-- [Disable CUDA here]
│   └── src
│       └── main.rs                    <-- [Host code goes here]
└── methods
    ├── Cargo.toml
    ├── build.rs                       <-- [Reproducible guest builds stuff here]
    ├── guest
    │   ├── Cargo.toml
    │   └── src
    │       └── method_name.rs         <-- [Guest code goes here]
    └── src
        └── lib.rs
```

## License

This repository is licensed under the GNU AGPLv3 or any later version. See [LICENSE.txt](./LICENSE.txt).
