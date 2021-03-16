FROM rust:1.50-alpine as builder

ENV WORKDIR /code
WORKDIR ${WORKDIR}

RUN rustup target add x86_64-unknown-linux-musl && \
  apk add --no-cache musl-dev perl make gcc && \
  USER=root cargo new another-rust-load-balancer

WORKDIR ${WORKDIR}/another-rust-load-balancer

ADD Cargo.toml ./Cargo.toml
ADD Cargo.lock ./Cargo.lock

RUN cargo build --release

COPY src ./src

RUN cargo install --target x86_64-unknown-linux-musl --path .

FROM scratch

COPY --from=builder /usr/local/cargo/bin/another-rust-load-balancer /usr/bin/another-rust-load-balancer

CMD ["another-rust-load-balancer"]
