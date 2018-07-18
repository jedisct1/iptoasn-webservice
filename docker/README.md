# How to use

Assume you are in the repo's root folder:

```sh
docker build -t iptoasn -f docker/Dockerfile .
docker run -itd \
           --name my-iptoasn \
           -p 80:53661 \
           iptoasn
```

Wait while iptoasn is downloading data, and then you can do `curl` requests as you used to:

```sh
curl 127.0.0.1:80/v1/as/ip/8.8.8.8
```

## Setting service parameters

Listen port and database URL can be specified by environment variables:

```sh
docker run -itd \
           --name my-iptoasn \
           -e IPTOASN_PORT=10000 \
           -e IPTOASN_DBURL='http://your-database-url.com' \
           -p 80:10000 \
           iptoasn
```

## Use as a binary

```sh
docker run -it --rm iptoasn --help
```
