FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    curl \
    bash \
    osmium-tool \
    python3 \
    python3-pip \
    cron \
  && pip3 install --no-cache-dir --break-system-packages \
    osmium \
    psycopg2-binary \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN mkdir -p /data /scripts /var/log \
  && touch /var/log/sync_osm.log \
  && chmod 0666 /var/log/sync_osm.log \
  && printf '0 18 * * * root DATABASE_URL=$DATABASE_URL /bin/bash /scripts/sync_osm.sh >> /var/log/sync_osm.log 2>&1\n' > /etc/cron.d/sync-osm \
  && chmod 0644 /etc/cron.d/sync-osm

CMD ["cron", "-f"]
