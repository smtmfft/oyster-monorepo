![Marlin Oyster Logo](./logo.svg)

# Raw Proxies

This repository contains raw IP proxies used to tunnel IP traffic through a vsock interface. They are primarily used in the tuna family of images. This repository contains the following proxies:
- ip-to-vsock-raw-incoming
- ip-to-vsock-raw-outgoing
- vsock-to-ip-raw-incoming
- vsock-to-ip-raw-outgoing

## Build

```bash
cargo build --release
```

## ip-to-vsock-raw-incoming

The ip-to-vsock-raw-incoming proxy listens on a netfilter queue for raw packets and proxies them to a fixed vsock address. Meant to be used in conjunction with [vsock-to-ip-raw-incoming](#vsock-to-ip-raw-incoming) proxy and iptables rules to intercept packets and redirect them into a netfilter queue.

### Prebuilt binaries

amd64: http://public.artifacts.marlin.pro/projects/enclaves/ip-to-vsock-raw-incoming_v1.0.0_linux_amd64

arm64: http://public.artifacts.marlin.pro/projects/enclaves/ip-to-vsock-raw-incoming_v1.0.0_linux_arm64

### Usage

```bash
$ ./target/release/ip-to-vsock-raw-incoming --help
Usage: ip-to-vsock-raw-incoming --vsock-addr <VSOCK_ADDR> --queue-num <QUEUE_NUM>

Options:
  -v, --vsock-addr <VSOCK_ADDR>  vsock address to forward packets to <cid:port>
  -q, --queue-num <QUEUE_NUM>    nfqueue number of the listener <num>
  -h, --help                     Print help
  -V, --version                  Print version
```

## vsock-to-ip-raw-incoming

The vsock-to-ip-raw-incoming proxy listens on a vsock address for raw packets and proxies them to a fixed network device. Meant to be used in conjunction with [ip-to-vsock-raw-incoming](#ip-to-vsock-raw-incoming) proxy to receive raw packets.

### Prebuilt binaries

amd64: http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip-raw-incoming_v1.0.0_linux_amd64

arm64: http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip-raw-incoming_v1.0.0_linux_arm64

### Usage

```bash
$ ./target/release/vsock-to-ip-raw-incoming --help
Usage: vsock-to-ip-raw-incoming --vsock-addr <VSOCK_ADDR> --device <DEVICE>

Options:
  -v, --vsock-addr <VSOCK_ADDR>  vsock address to listen on <cid:port>
  -d, --device <DEVICE>          network device to forward packets on
  -h, --help                     Print help
  -V, --version                  Print version
```

## ip-to-vsock-raw-outgoing

The ip-to-vsock-raw-outgoing proxy listens on a netfilter queue for raw packets and proxies them to a fixed vsock address. Meant to be used in conjunction with [vsock-to-ip-raw-outgoing](#vsock-to-ip-raw-outgoing) proxy and iptables rules to intercept packets and redirect them into a netfilter queue.

### Prebuilt binaries

amd64: http://public.artifacts.marlin.pro/projects/enclaves/ip-to-vsock-raw-outgoing_v1.0.0_linux_amd64

arm64: http://public.artifacts.marlin.pro/projects/enclaves/ip-to-vsock-raw-outgoing_v1.0.0_linux_arm64

### Usage

```bash
$ ./target/release/ip-to-vsock-raw-outgoing --help
Usage: ip-to-vsock-raw-outgoing --vsock-addr <VSOCK_ADDR> --queue-num <QUEUE_NUM>

Options:
  -v, --vsock-addr <VSOCK_ADDR>  vsock address to forward packets to <cid:port>
  -q, --queue-num <QUEUE_NUM>    nfqueue number of the listener <num>
  -h, --help                     Print help
  -V, --version                  Print version
```

## vsock-to-ip-raw-outgoing

The vsock-to-ip-raw-outgoing proxy listens on a vsock address for raw packets and proxies them to a fixed network device. Meant to be used in conjunction with [ip-to-vsock-raw-outgoing](#ip-to-vsock-raw-outgoing) proxy to receive raw packets.

### Prebuilt binaries

amd64: http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip-raw-outgoing_v1.0.0_linux_amd64

arm64: http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip-raw-outgoing_v1.0.0_linux_arm64

### Usage

```bash
$ ./target/release/vsock-to-ip-raw-outgoing --help
Usage: vsock-to-ip-raw-outgoing --vsock-addr <VSOCK_ADDR>

Options:
  -v, --vsock-addr <VSOCK_ADDR>  vsock address to listen on <cid:port>
  -h, --help                     Print help
  -V, --version                  Print version
```
