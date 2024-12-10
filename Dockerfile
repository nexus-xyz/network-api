FROM rust:latest

# Install system dependencies
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git-all \
    cmake \
    curl \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Set environment variables
ENV NEXUS_HOME=/root/.nexus
ENV NONINTERACTIVE=1

# Create NEXUS_HOME directory
RUN mkdir -p $NEXUS_HOME

# Copy prover ID if provided as a build arg
ARG PROVER_ID
RUN if [ ! -z "$PROVER_ID" ]; then \
        if [ ${#PROVER_ID} -eq "28" ]; then \
            echo "$PROVER_ID" > $NEXUS_HOME/prover-id && \
            echo "Prover id saved to $NEXUS_HOME/prover-id."; \
        else \
            echo "Unable to validate $PROVER_ID. Please make sure the full prover id is copied."; \
            exit 1; \
        fi \
    fi

# Copy the cli directory and proto files
COPY clients/cli ./clients/cli
COPY proto ./proto

# Build the project
RUN cd clients/cli && cargo build --release --bin prover

# Set the entrypoint to run the prover
ENTRYPOINT ["clients/cli/target/release/prover", "beta.orchestrator.nexus.xyz"]
