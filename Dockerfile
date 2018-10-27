FROM rust:latest

RUN rustup component add clippy-preview

COPY ./ /run/
WORKDIR /run

RUN cargo build && cargo build --release