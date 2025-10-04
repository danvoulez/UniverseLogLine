# syntax=docker/dockerfile:1.6

########################################
# Build ALL services stage
########################################
FROM ubuntu:22.04 AS builder

# Install Rust manually to ensure latest version
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Install latest Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY . .

# Build ALL LogLine services at once
RUN cargo build --release --bin logline-gateway && \
    cargo build --release --bin logline-id && \
    cargo build --release --bin logline-timeline && \
    cargo build --release --bin logline-rules && \
    cargo build --release --bin logline-engine && \
    cargo build --release --bin logline-federation

########################################
# Service selection stage
########################################
FROM builder AS service-selector
ARG SERVICE=logline-id
RUN cp /app/target/release/${SERVICE} /app/service-binary

########################################
# Runtime stage
########################################
FROM debian:bookworm-slim AS runtime

ARG SERVICE=logline-id
ARG SERVICE_CMD=${SERVICE}
ARG SERVICE_PORT=8080
ARG HEALTH_PATH=/health

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        libpq5 \
        curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false logline

COPY --from=service-selector /app/service-binary /usr/local/bin/service
RUN chown logline:logline /usr/local/bin/service && \
    chmod +x /usr/local/bin/service

USER logline

EXPOSE ${SERVICE_PORT}

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://localhost:${SERVICE_PORT}${HEALTH_PATH} || exit 1

CMD ["/usr/local/bin/service"]
