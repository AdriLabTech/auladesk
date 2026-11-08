# syntax=docker/dockerfile:1
# Compilación reproducible de AulaDesk en Linux (Debian 12). NO ejecuta la app (es de escritorio): genera los instaladores.
#
#   docker build --target artifacts -o ./instaladores .     # -> ./instaladores/{deb,rpm}/ y el binario
#   docker build --target check .                           # tipos de Svelte + pruebas de Rust
#   docker build --build-arg BUNDLES=deb,rpm,appimage --target artifacts -o ./instaladores .   # AppImage (necesita red: descarga linuxdeploy)
FROM rust:1-bookworm AS base
ARG NODE_MAJOR=20
RUN apt-get update && apt-get install -y --no-install-recommends \
      curl ca-certificates gnupg build-essential pkg-config file xdg-utils \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev libssl-dev libdbus-1-dev \
 && curl -fsSL https://deb.nodesource.com/setup_${NODE_MAJOR}.x | bash - && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .

FROM base AS check
RUN npm run check && cd src-tauri && cargo test --lib

FROM base AS build
ARG BUNDLES=deb,rpm
ENV CI=true APPIMAGE_EXTRACT_AND_RUN=1
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    npx tauri build --bundles ${BUNDLES}

FROM scratch AS artifacts
COPY --from=build /app/src-tauri/target/release/bundle/ /
COPY --from=build /app/src-tauri/target/release/auladesk /auladesk
