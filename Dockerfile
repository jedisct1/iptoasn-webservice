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
  && cargo build --release \
  && mv target/release/iptoasn-webservice /usr/bin/iptoasn-webservice \
  && rm -rf  ~/.cargo \
            /var/cache/apk/* \
            /tmp/* \
  && apk del .build-rust

RUN adduser -D app
USER app

ENTRYPOINT /usr/bin/iptoasn-webservice --listen 0.0.0.0:10000
