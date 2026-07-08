# Multi-stage build producing both edamame binaries.
FROM rust:1.95-slim-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin edamame-core --bin edamame-node

FROM debian:bookworm-slim AS runtime
# reqwest is built with default-features = false (http-only, no TLS), so no
# ca-certificates / openssl runtime is required.

FROM runtime AS combined
COPY --from=builder /build/target/release/edamame-core /usr/local/bin/edamame-core
COPY --from=builder /build/target/release/edamame-node /usr/local/bin/edamame-node

FROM runtime AS core
COPY --from=builder /build/target/release/edamame-core /usr/local/bin/edamame-core
ENTRYPOINT ["edamame-core"]

FROM runtime AS node
COPY --from=builder /build/target/release/edamame-node /usr/local/bin/edamame-node
ENTRYPOINT ["edamame-node"]
