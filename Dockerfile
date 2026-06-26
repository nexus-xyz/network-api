FROM alpine:3.22.0

RUN apk update && \
    apk add curl && \
    curl -sSf https://cli.nexus.xyz/ -o install.sh && \
    chmod +x install.sh && \
    NONINTERACTIVE=1 ./install.sh

ENTRYPOINT ["/root/.nexus/bin/nexus-compute-cli"]
