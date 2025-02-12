FROM rust:1.79-slim as builder

WORKDIR /usr/src/million_checkboxes

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --bin million_checkboxes --release

FROM debian:bookworm-slim

COPY --from=builder /usr/src/million_checkboxes/target/release/million_checkboxes /usr/local/bin/million_checkboxes

EXPOSE 8900

# executes from path /
CMD ["million_checkboxes"]