![Marlin Oyster Logo](./logo.svg)

# Init server

Initialization server that is used to feed in initialization parameters to the enclave, primarily the job id and the IP of the instance. Note that the init server is fully controlled by the operator and is not guaranteed to provide accurate data, the enclave needs to be designed accordingly. Meant to be used with [vet](https://github.com/marlinprotocol/vet), a curl-like utility that works over vsocks.

## Build

```bash
cargo build --release
```

## Prebuilt binaries

amd64: http://public.artifacts.marlin.pro/projects/enclaves/oyster-init-server_v1.0.0_linux_amd64

arm64: http://public.artifacts.marlin.pro/projects/enclaves/oyster-init-server_v1.0.0_linux_arm64

## Usage

```bash
$ ./target/release/oyster-init-server --help
Usage: oyster-init-server --vsock-addr <VSOCK_ADDR> --job-id <JOB_ID>

Options:
  -v, --vsock-addr <VSOCK_ADDR>  vsock address to listen on <cid:port>
  -j, --job-id <JOB_ID>          job id served by the enclave
  -h, --help                     Print help
  -V, --version                  Print version
```

## Endpoints

### Job Id

##### Endpoint

```
/oyster/job
```

##### Example

```
$ vet --url 3:1500/oyster/job
0x1234567812345678123456781234567812345678123456781234567812345678
```

### IP

##### Endpoint

```
/instance/ip
```

##### Example

```
$ vet --url 3:1500/instance/ip
192.168.0.1
```
