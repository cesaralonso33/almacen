#!/usr/bin/env bash
# Vacía el inventario de expedientes y su operación asociada.
#
# ADVERTENCIA: borra expedientes, pedidos, entregas, custodias, movimientos,
# reservas, descartes, devoluciones, transferencias, incidencias, eventos de
# sincronización y claves de idempotencia. Conserva usuarios, roles, ubicaciones
# y la bitácora de auditoría.
#
# Uso: ./mantenimiento/limpiar-expedientes.sh
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

cat <<'AVISO'
Esto vaciará el inventario y toda su operación:
  expedientes, pedidos, renglones, reservas, entregas, acuses, custodias,
  movimientos, descartes, devoluciones, transferencias e incidencias.
También reinicia los eventos de sincronización para que los clientes
reciban un snapshot vacío.
No se borran usuarios, roles, ubicaciones ni la auditoría.
AVISO

read -r -p "Escriba BORRAR para continuar: " confirmacion
if [ "$confirmacion" != "BORRAR" ]; then
    echo "cancelado"
    exit 1
fi

bash ./backup/backup.sh

echo "Deteniendo la API..."
docker compose stop api

echo "Vaciando tablas..."
docker compose exec -T postgres psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 <<'SQL'
BEGIN;

TRUNCATE TABLE
    acuses,
    descartes,
    reservas,
    entregas,
    custodias,
    movimientos,
    pedido_items,
    pedidos,
    devoluciones,
    transferencias,
    incidencias,
    expedientes
RESTART IDENTITY CASCADE;

TRUNCATE TABLE sync_events RESTART IDENTITY;
TRUNCATE TABLE operaciones_idempotentes;
UPDATE sync_devices SET ultima_secuencia = 0, ultima_sync = NULL;

COMMIT;
SQL

echo "Reiniciando la API..."
docker compose start api

echo "Listo. Los clientes recibirán un snapshot vacío al sincronizar."
echo "La auditoría se conservó como evidencia de lo ocurrido."
