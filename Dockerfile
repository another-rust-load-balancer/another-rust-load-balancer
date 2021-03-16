FROM rust:1.50-alpine as builder

ENV WORKDIR /code
WORKDIR ${WORKDIR}

RUN rustup target add x86_64-unknown-linux-musl && \
  apk add --no-cache musl-dev perl make gcc

ADD . .

RUN cargo build --release --target x86_64-unknown-linux-musl

FROM scratch as runner

COPY --from=builder /code/target/x86_64-unknown-linux-musl/release/another-rust-load-balancer /usr/bin/another-rust-load-balancer

CMD ["another-rust-load-balancer"]
