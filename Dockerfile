FROM rust:1.79 as builder

WORKDIR /usr/src/million_checkboxes

COPY . .

RUN cargo build --release

FROM ubuntu:22.04

COPY --from=builder /usr/src/million_checkboxes/target/release/million_checkboxes /usr/local/bin/million_checkboxes

EXPOSE 8900

CMD ["million_checkboxes"]