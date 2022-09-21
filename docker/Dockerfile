FROM alpine:3.16.2

COPY Cargo.* /tmp/iptoasn/
COPY src /tmp/iptoasn/src

WORKDIR /tmp/iptoasn

RUN apk add --update --no-cache ca-certificates \
                                libressl \
                                llvm-libunwind \
                                libgcc \
  && apk add --no-cache --virtual .build-rust \
    rust \
    cargo \
    libressl-dev \
  \
  && cargo build --release \
  && strip target/release/iptoasn-webservice \
  && mv target/release/iptoasn-webservice /usr/bin/iptoasn-webservice \
  && adduser -D app \
  \
  && apk del .build-rust \
  && rm -rf  ~/.cargo \
            /var/cache/apk/* \
            /tmp/*

COPY docker/iptoasn-entrypoint.sh /iptoasn-entrypoint.sh
RUN chmod +x /iptoasn-entrypoint.sh

USER app

ENTRYPOINT ["/iptoasn-entrypoint.sh"]
