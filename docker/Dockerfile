FROM alpine:3.18.2 AS builder

WORKDIR /tmp/iptoasn

RUN apk add --update --no-cache ca-certificates \
                                libressl \
                                llvm-libunwind \
                                libgcc \
  && apk add --no-cache --virtual .build-rust \
    rust \
    cargo \
    libressl-dev

COPY Cargo.* /tmp/iptoasn/
COPY src /tmp/iptoasn/src

RUN cargo build --release \
  && strip target/release/iptoasn-webservice

FROM alpine:3.18.2 as prod

RUN apk add --update --no-cache ca-certificates \
                                libressl \
                                llvm-libunwind \
                                libgcc \
    && adduser -D app

COPY --from=0 /tmp/iptoasn/target/release/iptoasn-webservice /usr/bin/iptoasn-webservice
COPY docker/iptoasn-entrypoint.sh /iptoasn-entrypoint.sh
RUN chmod +x /iptoasn-entrypoint.sh

USER app

ENTRYPOINT ["/iptoasn-entrypoint.sh"]
