#!/bin/sh

DEFAULT_PORT='53661'
DEFAULT_DBURL='https://iptoasn.com/data/ip2asn-combined.tsv.gz'

if [ $IPTOASN_PORT ] || [ $IPTOASN_DBURL]; then
  if ! [ $IPTOASN_PORT ]; then
    IPTOASN_PORT=$DEFAULT_PORT
  fi

  if ! [ $IPTOASN_DBURL ]; then
    IPTOASN_DBURL=$DEFAULT_DBURL
  fi

  exec /usr/bin/iptoasn-webservice --listen 0.0.0.0:"$IPTOASN_PORT" --dburl "$IPTOASN_DBURL"
else
  exec /usr/bin/iptoasn-webservice $@
fi
