FROM rust:1.100.0-alpine3.23 AS builder

RUN apk add --no-cache build-base

WORKDIR /tmp/iptoasn
COPY Cargo.* ./
COPY src ./src

RUN cargo build --release --locked \
  && strip target/release/iptoasn-webservice

FROM alpine:3.23

RUN apk add --no-cache ca-certificates libressl llvm-libunwind libgcc \
  && adduser -D app

COPY --from=builder /tmp/iptoasn/target/release/iptoasn-webservice /usr/bin/iptoasn-webservice
COPY docker/iptoasn-entrypoint.sh /iptoasn-entrypoint.sh
RUN chmod +x /iptoasn-entrypoint.sh

USER app

ENTRYPOINT ["/iptoasn-entrypoint.sh"]
