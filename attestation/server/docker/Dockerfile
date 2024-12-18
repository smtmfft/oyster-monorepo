FROM ubuntu:22.10
COPY target/release/oyster-attestation-server /app/oyster-attestation-server
COPY docker/id_ed25519.pub  /app/id_ed25519.pub
WORKDIR /app
ENTRYPOINT ["/app/oyster-attestation-server", "--ip-addr", "0.0.0.0:8080", "--pub-key", "/app/id_ed25519.pub"]