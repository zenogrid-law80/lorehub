# syntax=docker/dockerfile:1.7
FROM rust:1.91-slim-trixie AS lorehub-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    protobuf-compiler \
    libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build/lorehub
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/lorehub/target \
    cargo build --locked --release --bin lorehub \
    && cp target/release/lorehub /tmp/lorehub

FROM rust:1.91-slim-trixie
ARG LORE_VERSION=0.9.0
ADD --checksum=sha256:05a1890406ff400d265e43c58afef4da2fdcb23642483766eb35bb0d4b904a5a \
    https://github.com/EpicGames/lore/releases/download/v${LORE_VERSION}/lore-v${LORE_VERSION}-x86_64-unknown-linux-gnu.tar.gz \
    /tmp/lore.tar.gz
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    git \
    pkg-config \
    && tar -xzf /tmp/lore.tar.gz -C /usr/local/bin ./lore \
    && chmod 0755 /usr/local/bin/lore \
    && rm -f /tmp/lore.tar.gz \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 lorehub \
    && useradd --system --uid 10001 --gid lorehub --create-home lorehub \
    && mkdir -p /var/lib/lorehub /home/lorehub/.cargo \
    && chown -R lorehub:lorehub /var/lib/lorehub /home/lorehub
COPY --from=lorehub-builder /tmp/lorehub /usr/local/bin/lorehub
USER lorehub
WORKDIR /var/lib/lorehub
ENV LORE_BIN=/usr/local/bin/lore \
    LOREHUB_WORK_DIR=/var/lib/lorehub/work \
    RUST_LOG=lorehub=info
ENTRYPOINT ["/usr/local/bin/lorehub", "worker"]
