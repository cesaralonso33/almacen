#!/usr/bin/env bash
# Prepara el backend en un VPS: secretos, variante TLS, arranque y verificación.
# Uso:
#   ./preparar-vps.sh
#   ./preparar-vps.sh --admin-correo jefe@institucion.mx --admin-password 'Clave#Larga-2026'
#   ./preparar-vps.sh --http        # solo red cerrada o VPN
set -euo pipefail

cd "$(dirname "$0")"

MODO="tls"
SITIO_MANUAL=""
ADMIN_CORREO=""
ADMIN_PASSWORD=""
ADMIN_NOMBRE="Administrador"

uso() {
    cat <<'EOF'
Uso: ./preparar-vps.sh [opciones]

  --http                  Servir por HTTP sin TLS (solo red cerrada o VPN)
  --sitio DOMINIO         Nombre público (por omisión <ip>.sslip.io)
  --admin-correo CORREO   Crea el administrador inicial
  --admin-password CLAVE  Contraseña del administrador (mínimo 12)
  --admin-nombre NOMBRE   Nombre visible (por omisión "Administrador")
  -h, --help              Esta ayuda
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --http) MODO="http"; shift ;;
        --sitio) SITIO_MANUAL="${2:?falta el dominio}"; shift 2 ;;
        --admin-correo) ADMIN_CORREO="${2:?falta el correo}"; shift 2 ;;
        --admin-password) ADMIN_PASSWORD="${2:?falta la contraseña}"; shift 2 ;;
        --admin-nombre) ADMIN_NOMBRE="${2:?falta el nombre}"; shift 2 ;;
        -h | --help) uso; exit 0 ;;
        *) echo "Opción desconocida: $1" >&2; uso; exit 2 ;;
    esac
done

for herramienta in docker openssl curl; do
    command -v "$herramienta" >/dev/null 2>&1 || {
        echo "Falta $herramienta en el sistema" >&2
        exit 1
    }
done
docker compose version >/dev/null 2>&1 || {
    echo "Falta el plugin 'docker compose'" >&2
    exit 1
}

if [ ! -f .env ]; then
    cp .env.example .env
fi
chmod 600 .env

definir() {
    local clave="$1" valor="$2"
    if grep -q "^${clave}=" .env; then
        sed -i "s|^${clave}=.*|${clave}=${valor}|" .env
    else
        printf '%s=%s\n' "$clave" "$valor" >> .env
    fi
}

# Secretos: solo se generan si siguen vacíos o con el valor de ejemplo.
if ! grep -qE '^POSTGRES_PASSWORD=.{16,}' .env || grep -q '^POSTGRES_PASSWORD=cambie' .env; then
    definir POSTGRES_PASSWORD "$(openssl rand -hex 24)"
    echo "POSTGRES_PASSWORD generado"
fi
if ! grep -qE '^JWT_SECRET=.{32,}' .env || grep -q '^JWT_SECRET=cambie' .env; then
    definir JWT_SECRET "$(openssl rand -hex 32)"
    echo "JWT_SECRET generado"
fi

IP_PUBLICA="$(curl -fsS --max-time 5 https://api.ipify.org 2>/dev/null || true)"
if [ -z "$IP_PUBLICA" ]; then
    IP_PUBLICA="$(hostname -I 2>/dev/null | awk '{print $1}')"
fi
if [ -z "$IP_PUBLICA" ]; then
    echo "No fue posible determinar la IP; use --sitio <nombre>" >&2
    exit 1
fi

if [ "$MODO" = "http" ]; then
    SITIO="${SITIO_MANUAL:-$IP_PUBLICA}"
    definir SITIO "$SITIO"
    definir CADDYFILE "./Caddyfile.http"
    ESQUEMA="http"
else
    SITIO="${SITIO_MANUAL:-${IP_PUBLICA//./-}.sslip.io}"
    definir SITIO "$SITIO"
    definir CADDYFILE "./Caddyfile.publico"
    ESQUEMA="https"
fi

echo "Modo: $MODO"
echo "Sitio: $SITIO"

docker compose up -d --build

echo "Esperando a que responda $ESQUEMA://$SITIO/health ..."
listo=0
for _ in $(seq 1 45); do
    if curl -fsS --max-time 3 "$ESQUEMA://$SITIO/health" >/dev/null 2>&1; then
        listo=1
        break
    fi
    sleep 2
done

if [ "$listo" -ne 1 ]; then
    echo "La API no respondió todavía. Últimos registros de Caddy:" >&2
    docker compose logs --tail=40 caddy >&2 || true
    echo >&2
    echo "Si el proveedor bloquea 80/443, use: ./preparar-vps.sh --http" >&2
    exit 1
fi

echo "API disponible en $ESQUEMA://$SITIO"
echo "Verifique con: curl -fsS $ESQUEMA://$SITIO/health"

if [ "$MODO" = "http" ]; then
    echo "ADVERTENCIA: el tráfico no está cifrado. Restrinja el puerto 80 a la red" \
        "institucional o a una VPN."
fi

if [ -n "$ADMIN_CORREO" ] && [ -n "$ADMIN_PASSWORD" ]; then
    docker compose exec -T api env \
        ADMIN_CORREO="$ADMIN_CORREO" \
        ADMIN_PASSWORD="$ADMIN_PASSWORD" \
        ADMIN_NOMBRE="$ADMIN_NOMBRE" \
        /usr/local/bin/crear_admin
    echo "Administrador creado: $ADMIN_CORREO"
else
    echo "Cree la cuenta administradora con:"
    echo "  docker compose exec api env ADMIN_CORREO='jefe@institucion.mx' \\"
    echo "    ADMIN_PASSWORD='UnaClaveLargaYUnica#2026' /usr/local/bin/crear_admin"
fi

echo
echo "Respaldo sugerido en crontab -e:"
echo "  0 2,14 * * * cd $(pwd) && bash ./backup/backup.sh >> backup/registro.log 2>&1"
