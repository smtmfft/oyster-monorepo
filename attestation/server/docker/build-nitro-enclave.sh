docker build -t attestation-app -f docker/Dockerfile .

nitro-cli build-enclave \
    --docker-uri attestation-app:latest \
    --output-file attestation.eif

nitro-cli run-enclave \
    --eif-path attestation.eif \
    --cpu-count 2 \
    --memory 512 \
    --debug-mode && nitro-cli console --enclave-id $(nitro-cli describe-enclaves | jq -r '.[0].EnclaveID')

nitro-cli describe-enclaves

sudo socat TCP-LISTEN:8080,reuseaddr,fork VSOCK-CONNECT:62:8080