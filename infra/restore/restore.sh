#!/usr/bin/env bash
# Restauración de un respaldo. Crea un respaldo previo y detiene la API durante el proceso.
# Uso: ./restore/restore.sh backup/salidas/expedientes_20260101T030000Z.dump
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -f .env ]; then
    set -a
    # shellcheck disable=SC1091
    . ./.env
    set +a
fi

archivo="${1:-}"
if [ -z "$archivo" ]; then
    echo "uso: $0 <respaldo.dump>" >&2
    exit 2
fi
if [ ! -f "$archivo" ]; then
    echo "no existe el archivo $archivo" >&2
    exit 2
fi

echo "Se reemplazará la base ${POSTGRES_DB} con $archivo."
read -r -p "Escriba RESTAURAR para continuar: " confirmacion
if [ "$confirmacion" != "RESTAURAR" ]; then
    echo "cancelado"
    exit 1
fi

bash ./backup/backup.sh

docker compose stop api
docker compose exec -T postgres dropdb -U "$POSTGRES_USER" --force --if-exists "$POSTGRES_DB"
docker compose exec -T postgres createdb -U "$POSTGRES_USER" "$POSTGRES_DB"
docker compose exec -T postgres pg_restore -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists < "$archivo" || true
docker compose start api

echo "restauración terminada. Verifique /health y la bitácora de auditoría."
