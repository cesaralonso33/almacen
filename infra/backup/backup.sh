#!/usr/bin/env bash
# Respaldo lógico de PostgreSQL con rotación por antigüedad.
# Uso: ./backup/backup.sh
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -f .env ]; then
    set -a
    # shellcheck disable=SC1091
    . ./.env
    set +a
fi

: "${POSTGRES_USER:?Defina POSTGRES_USER en infra/.env}"
: "${POSTGRES_DB:?Defina POSTGRES_DB en infra/.env}"

mkdir -p backup/salidas

marca="$(date -u +%Y%m%dT%H%M%SZ)"
archivo="backup/salidas/expedientes_${marca}.dump"

docker compose exec -T postgres \
    pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom > "$archivo"

echo "respaldo creado: $archivo ($(wc -c < "$archivo") bytes)"

retencion="${RETENCION_DIAS:-30}"
find backup/salidas -name 'expedientes_*.dump' -mtime +"$retencion" -delete
echo "retención aplicada: $retencion días"
