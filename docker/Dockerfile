FROM alpine:edge

COPY ./ /tmp

WORKDIR /tmp

RUN apk update \
  && apk add --no-cache ca-certificates \
                        libressl \
                        llvm-libunwind \
  && apk add --no-cache --virtual .build-rust \
    rust \
    cargo \
    libressl-dev \
  \
  && cargo build --release \
  && strip target/release/iptoasn-webservice \
  && mv target/release/iptoasn-webservice /usr/bin/iptoasn-webservice \
  && mv docker/iptoasn-entrypoint.sh /iptoasn-entrypoint.sh \
  && chmod +x /iptoasn-entrypoint.sh \
  \
  && rm -rf  ~/.cargo \
            /var/cache/apk/* \
            /tmp/* \
  && apk del .build-rust

RUN adduser -D app
USER app

ENTRYPOINT ["/iptoasn-entrypoint.sh"]
