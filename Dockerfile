# syntax=docker/dockerfile:1.6

########################################
# Build stage
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

ARG SERVICE=logline-id

WORKDIR /app

COPY . .

RUN cargo build --release --bin ${SERVICE}

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

COPY --from=builder /app/target/release/${SERVICE} /usr/local/bin/${SERVICE}
RUN chown logline:logline /usr/local/bin/${SERVICE}

USER logline

EXPOSE ${SERVICE_PORT}

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://localhost:${SERVICE_PORT}${HEALTH_PATH} || exit 1

CMD ["${SERVICE_CMD}"]
