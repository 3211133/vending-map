FROM rust:1.88-bookworm AS builder
WORKDIR /app

COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src
COPY backend/src ./src

RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
        python3 python3-pip \
        tesseract-ocr tesseract-ocr-jpn tesseract-ocr-eng \
        libgl1 libglib2.0-0 \
    && rm -rf /var/lib/apt/lists/*

RUN pip3 install --no-cache-dir --break-system-packages \
    pytesseract Pillow opencv-python-headless numpy

COPY --from=builder /app/target/release/vending-map-backend /usr/local/bin/vending-map-backend
COPY scripts/analyze_image.py /app/scripts/analyze_image.py

EXPOSE 8080

CMD ["vending-map-backend"]
