FROM caddy:2.7.6-builder AS builder

RUN xcaddy build \
    --with github.com/mholt/caddy-l4 \
    --with github.com/caddy-dns/cloudflare

FROM caddy:2.7.6

COPY --from=builder /usr/bin/caddy /usr/bin/caddy

RUN apk add curl

CMD ["caddy", "run"]
