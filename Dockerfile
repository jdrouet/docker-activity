FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev

ENV USER=root

WORKDIR /code
RUN cargo init
COPY Cargo.toml Cargo.lock /code/
RUN cargo fetch
COPY src/main.rs /code/src/main.rs
RUN cargo build --release --offline

FROM alpine

COPY --from=builder /code/target/release/docker-activity /docker-activity

ENTRYPOINT ["/docker-activity"]

