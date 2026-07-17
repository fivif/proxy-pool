# ── Stage 1: Build ──
FROM rust:1.89-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release 2>/dev/null \
    && rm -rf src

COPY src ./src
RUN cargo build --release

# ── Stage 2: Runtime (Debian glibc) ──
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && apt-get clean

ENV TZ=Asia/Shanghai RUST_LOG=info

COPY --from=builder /app/target/release/proxy-pool /usr/local/bin/proxy-pool

EXPOSE 3000
ENTRYPOINT ["proxy-pool"]
