#!/usr/bin/env bash
# Genera los instaladores de Linux dentro de Docker y los deja en ./instaladores
set -euo pipefail
cd "$(dirname "$0")/.."
docker build --target artifacts --build-arg "BUNDLES=${BUNDLES:-deb,rpm}" -o ./instaladores .
echo "Listo: $(find instaladores -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) | tr '\n' ' ')"
