FROM ubuntu:22.04

RUN apt-get update -y
RUN apt-get install apt-utils -y
RUN apt-get install python3 python3-pip net-tools iptables curl cgroup-tools iproute2 wget clang libc++-dev libc++abi-dev -y

WORKDIR /app

# Supervisord to manage programs
RUN wget -O supervisord http://public.artifacts.marlin.pro/projects/enclaves/supervisord_master_linux_amd64
RUN chmod +x supervisord

# Transparent proxy component inside the enclave to enable outgoing connections
RUN wget -O ip-to-vsock-transparent http://public.artifacts.marlin.pro/projects/enclaves/ip-to-vsock-transparent_v1.0.0_linux_amd64
RUN chmod +x ip-to-vsock-transparent

# Key generator to generate static ed25519 keys
RUN wget -O keygen-ed25519 http://public.artifacts.marlin.pro/projects/enclaves/keygen-ed25519_v1.0.0_linux_amd64
RUN chmod +x keygen-ed25519

# Key generator to generate static secp256k1 keys
RUN wget -O keygen-secp256k1 http://public.artifacts.marlin.pro/projects/enclaves/keygen-secp256k1_v1.0.0_linux_amd64
RUN chmod +x keygen-secp256k1

# Attestation server inside the enclave that generates attestations
RUN wget -O attestation-server http://public.artifacts.marlin.pro/projects/enclaves/attestation-server_v1.0.0_linux_amd64
RUN chmod +x attestation-server

# Proxy to expose attestation server outside the enclave
RUN wget -O vsock-to-ip http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip_v1.0.0_linux_amd64
RUN chmod +x vsock-to-ip

# DNSproxy to provide DNS services inside the enclave
RUN wget -O dnsproxy http://public.artifacts.marlin.pro/projects/enclaves/dnsproxy_v0.46.5_linux_amd64
RUN chmod +x dnsproxy

# Supervisord config
COPY supervisord.conf /etc/supervisord.conf

# setup.sh script that will act as entrypoint
COPY setup.sh ./
RUN chmod +x setup.sh

# oyster serverless executor inside the enclave that executes web3 jobs
COPY . ./oyster-serverless-executor 
RUN chmod +x oyster-serverless-executor

# Entry point
ENTRYPOINT [ "/app/setup.sh" ]