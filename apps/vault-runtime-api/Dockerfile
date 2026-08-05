FROM rust:1.97-bookworm AS builder

WORKDIR /app

COPY apps/vault-runtime-api /app/apps/vault-runtime-api
COPY src-tauri/crates /app/src-tauri/crates
COPY src-tauri/scripts /app/src-tauri/scripts

RUN cargo build --release --manifest-path /app/apps/vault-runtime-api/Cargo.toml

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates python3 python3-venv libreoffice-core libreoffice-impress \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/apps/vault-runtime-api/target/release/vault-runtime-api /usr/local/bin/vault-runtime-api
COPY --from=builder /app/src-tauri/scripts /app/src-tauri/scripts

ENV VAULT_RUNTIME_PORT=8787
ENV VAULT_RUNTIME_BIND_HOST=0.0.0.0
ENV VAULT_RUNTIME_DATA_DIR=/var/lib/cheersai-vault/runtime-data
ENV VAULT_OCR_SCRIPT=/app/src-tauri/scripts/pdf_ocr.py

EXPOSE 8787

CMD ["vault-runtime-api"]
