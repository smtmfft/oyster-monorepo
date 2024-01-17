# base image
FROM alpine:3.18

# install dependency tools
RUN apk add --no-cache net-tools iptables iproute2 wget

# working directory
WORKDIR /app

# supervisord to manage programs
RUN wget -O supervisord http://public.artifacts.marlin.pro/projects/enclaves/supervisord_master_linux_amd64
RUN chmod +x supervisord

# transparent proxy component inside the enclave to enable outgoing connections
RUN wget -O ip-to-vsock-transparent http://public.artifacts.marlin.pro/projects/enclaves/ip-to-vsock-transparent_v1.0.0_linux_amd64
RUN chmod +x ip-to-vsock-transparent

# key generator to generate static keys
RUN wget -O keygen-ed25519 http://public.artifacts.marlin.pro/projects/enclaves/keygen-ed25519_v1.0.0_linux_amd64
RUN chmod +x keygen-ed25519

# attestation server inside the enclave that generates attestations
RUN wget -O attestation-server http://public.artifacts.marlin.pro/projects/enclaves/attestation-server_v1.0.0_linux_amd64
RUN chmod +x attestation-server

# proxy to expose attestation server outside the enclave
RUN wget -O vsock-to-ip http://public.artifacts.marlin.pro/projects/enclaves/vsock-to-ip_v1.0.0_linux_amd64
RUN chmod +x vsock-to-ip

# dnsproxy to provide DNS services inside the enclave
RUN wget -O dnsproxy http://public.artifacts.marlin.pro/projects/enclaves/dnsproxy_v0.46.5_linux_amd64
RUN chmod +x dnsproxy

# supervisord config
COPY supervisord.conf /etc/supervisord.conf

# setup.sh script that will act as entrypoint
COPY setup.sh ./
RUN chmod +x setup.sh

# your custom setup goes here

# key generator to generate secp256k1 keys
RUN wget -O keygen-secp256k1 http://public.artifacts.marlin.pro/projects/enclaves/keygen-secp256k1_v1.0.0_linux_amd64
RUN chmod +x keygen-secp256k1

# secp256k1 attestation server
RUN wget -O attestation-server-secp256k1 http://public.artifacts.marlin.pro/projects/enclaves/attestation-server-secp256k1_v1.0.0_linux_amd64
RUN chmod +x attestation-server-secp256k1

# attestation verifier
RUN wget -O attestation-verifier http://public.artifacts.marlin.pro/projects/enclaves/attestation-verifier_v1.0.0_linux_amd64
RUN chmod +x attestation-verifier

# entry point
ENTRYPOINT [ "/app/setup.sh" ]
