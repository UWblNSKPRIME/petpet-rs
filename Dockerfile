FROM rust:1-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=builder /app/target/release/petpet-rs /app/petpet-rs

EXPOSE 3000
ENTRYPOINT ["/app/petpet-rs"]
