# to be able to use `File::with_options` we need to use nightly build
FROM ghcr.io/rust-lang/rust:nightly-alpine AS base

RUN apk add --no-cache musl-dev

ENV USER=root

WORKDIR /code
RUN cargo init
COPY Cargo.toml Cargo.lock /code/
RUN cargo fetch
COPY src/*.rs /code/src/
COPY src/exporter /code/src/exporter
COPY src/format /code/src/format

FROM base AS dev

RUN cargo install cargo-hack

FROM base AS builder

RUN cargo build --release --offline

FROM alpine

COPY --from=builder /code/target/release/docker-activity /docker-activity

ENTRYPOINT ["/docker-activity"]

