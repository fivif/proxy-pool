# ── Stage 1: Build ──
FROM rust:1.89-alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig openssl-dev

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# 预下载依赖（利用 Docker 层缓存）
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release 2>/dev/null; rm -rf src

COPY src ./src
RUN cargo build --release

# ── Stage 2: Runtime ──
FROM alpine:3.21

RUN apk add --no-cache ca-certificates tzdata

ENV TZ=Asia/Shanghai \
    RUST_LOG=info \
    PROXY_CHECK_INTERVAL=60 \
    PROXY_FETCH_INTERVAL=300 \
    PROXY_MAX_POOL=5000 \
    PROXY_VALIDATION_CONCURRENCY=256 \
    PROXY_VALIDATION_TIMEOUT=8

COPY --from=builder /app/target/release/proxy-pool /usr/local/bin/proxy-pool

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD wget -qO- http://localhost:3000/health || exit 1

ENTRYPOINT ["proxy-pool"]
