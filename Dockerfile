FROM rust:1.86.0-alpine AS builder
# build project
RUN apk add musl-dev
WORKDIR /usr/src/
RUN USER=root cargo new hc-operator
WORKDIR /usr/src/hc-operator
COPY Cargo.toml .
COPY src ./src
RUN cargo build --release

# Bundle Stage
FROM alpine as final
COPY --from=builder /usr/src/hc-operator/target/release/hc-operator .
CMD ["./hc-operator"]
