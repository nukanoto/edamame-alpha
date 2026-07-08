# Multi-stage build producing both edamame binaries in one image.
# The compose file selects which binary to run via `command:`.
FROM rust:1.95-slim-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin edamame-core --bin edamame-node

FROM debian:bookworm-slim
# reqwest is built with default-features = false (http-only, no TLS), so no
# ca-certificates / openssl runtime is required.
COPY --from=builder /build/target/release/edamame-core /usr/local/bin/edamame-core
COPY --from=builder /build/target/release/edamame-node /usr/local/bin/edamame-node
