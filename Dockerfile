FROM rust:1.89-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata && apt-get clean
ENV TZ=Asia/Shanghai RUST_LOG=info
COPY --from=builder /app/target/release/proxy-pool /usr/local/bin/proxy-pool
EXPOSE 3000
ENTRYPOINT ["proxy-pool"]
