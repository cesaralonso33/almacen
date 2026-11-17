#!/usr/bin/env bash
# Prepara el backend en un VPS: secretos, variante TLS, arranque y verificación.
# Uso:
#   ./preparar-vps.sh
#   ./preparar-vps.sh --admin-correo jefe@institucion.mx --admin-password 'Clave#Larga-2026'
#   ./preparar-vps.sh --http        # solo red cerrada o VPN
#   ./preparar-vps.sh --portainer   # genera variables para el stack de Portainer
set -euo pipefail

cd "$(dirname "$0")"

MODO="tls"
SITIO_MANUAL=""
ADMIN_CORREO=""
ADMIN_PASSWORD=""
ADMIN_NOMBRE="Administrador"
PORTAINER=0

uso() {
    cat <<'EOF'
Uso: ./preparar-vps.sh [opciones]

  --http                  Servir por HTTP sin TLS (solo red cerrada o VPN)
  --portainer             Genera portainer.env y no levanta el stack localmente
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
        --portainer) PORTAINER=1; shift ;;
        --sitio) SITIO_MANUAL="${2:?falta el dominio}"; shift 2 ;;
        --admin-correo) ADMIN_CORREO="${2:?falta el correo}"; shift 2 ;;
        --admin-password) ADMIN_PASSWORD="${2:?falta la contraseña}"; shift 2 ;;
        --admin-nombre) ADMIN_NOMBRE="${2:?falta el nombre}"; shift 2 ;;
        -h | --help) uso; exit 0 ;;
        *) echo "Opción desconocida: $1" >&2; uso; exit 2 ;;
    esac
done

SITIO_MANUAL="${SITIO_MANUAL#http://}"
SITIO_MANUAL="${SITIO_MANUAL#https://}"
SITIO_MANUAL="${SITIO_MANUAL%%/*}"

if [ "$PORTAINER" -eq 1 ] && { [ -n "$ADMIN_CORREO" ] || [ -n "$ADMIN_PASSWORD" ]; }; then
    echo "ERROR: --portainer no crea el administrador." >&2
    echo "Genere las variables, despliegue el stack y ejecute crear_admin desde la consola" >&2
    echo "del contenedor api en Portainer." >&2
    exit 2
fi

for herramienta in openssl curl; do
    command -v "$herramienta" >/dev/null 2>&1 || {
        echo "Falta $herramienta en el sistema" >&2
        exit 1
    }
done

# `--portainer` solo genera secretos y variables; el stack lo despliega la UI.
if [ "$PORTAINER" -eq 0 ]; then
    command -v docker >/dev/null 2>&1 || {
        echo "Falta docker en el sistema" >&2
        exit 1
    }
    docker compose version >/dev/null 2>&1 || {
        echo "Falta el plugin 'docker compose'" >&2
        exit 1
    }
fi

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

valor() {
    sed -n "s/^$1=//p" .env | tail -1
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

IP_PUBLICA=""
if [ -z "$SITIO_MANUAL" ]; then
    IP_PUBLICA="$(curl -fsS --max-time 5 https://api.ipify.org 2>/dev/null || true)"
    if [ -z "$IP_PUBLICA" ]; then
        IP_PUBLICA="$(hostname -I 2>/dev/null | awk '{print $1}')"
    fi
    if [ -z "$IP_PUBLICA" ]; then
        echo "No fue posible determinar la IP; use --sitio <nombre>" >&2
        exit 1
    fi
fi

if [ "$MODO" = "http" ]; then
    SITIO="${SITIO_MANUAL:-$IP_PUBLICA}"
    definir SITIO "$SITIO"
    definir CADDYFILE "./Caddyfile.http"
    ESQUEMA="http"
else
    if [ -n "$SITIO_MANUAL" ]; then
        SITIO="$SITIO_MANUAL"
    else
        SITIO="${IP_PUBLICA//./-}.sslip.io"
    fi
    definir SITIO "$SITIO"
    definir CADDYFILE "./Caddyfile.publico"
    ESQUEMA="https"
fi

echo "Modo: $MODO"
echo "Sitio: $SITIO"

if [ "$MODO" = "tls" ]; then
    echo "Requisitos TLS: DNS de $SITIO debe apuntar a este VPS; puertos TCP 80 y 443 abiertos"
    echo "en el firewall del VPS y del proveedor. No se cambiará automáticamente a HTTP."
fi

if [ "$PORTAINER" -eq 1 ]; then
    infra_abs="$(pwd -P)"
    caddy_valor="$(valor CADDYFILE)"
    caddy_valor="${caddy_valor:-./Caddyfile}"
    case "$caddy_valor" in
        /*) caddy_abs="$caddy_valor" ;;
        *) caddy_abs="$infra_abs/${caddy_valor#./}" ;;
    esac
    certs_valor="$(valor CERTS_DIR)"
    certs_valor="${certs_valor:-./certs}"
    case "$certs_valor" in
        /*) certs_abs="$certs_valor" ;;
        *) certs_abs="$infra_abs/${certs_valor#./}" ;;
    esac
    api_image="$(valor API_IMAGE)"
    api_image="${api_image:-expedientes-api:local}"
    bind_ip="$(valor BIND_IP)"
    bind_ip="${bind_ip:-0.0.0.0}"

    if [ ! -f "$caddy_abs" ]; then
        echo "ERROR: no existe el Caddyfile: $caddy_abs" >&2
        exit 1
    fi
    mkdir -p "$certs_abs"
    case "$caddy_abs" in
        */Caddyfile)
            if [ ! -f "$certs_abs/tls.crt" ] || [ ! -f "$certs_abs/tls.key" ]; then
                echo "ADVERTENCIA: Caddyfile espera $certs_abs/tls.crt y tls.key." >&2
            fi
            ;;
    esac

    archivo="portainer.env"
    {
        echo "# Variables para infra/docker-compose.portainer.yml"
        echo "# Contiene secretos: no versionar; eliminar después de cargarlo en Portainer."
        echo "# Las rutas son absolutas para que el demonio Docker las resuelva en el host."
        printf 'SITIO=%s\n' "$SITIO"
        printf 'CADDYFILE=%s\n' "$caddy_abs"
        printf 'CERTS_DIR=%s\n' "$certs_abs"
        printf 'API_IMAGE=%s\n' "$api_image"
        printf 'BIND_IP=%s\n' "$bind_ip"
        for clave in \
            POSTGRES_DB POSTGRES_USER POSTGRES_PASSWORD JWT_SECRET \
            ACCESS_TTL_MINUTOS REFRESH_TTL_MINUTOS IDLE_TTL_MINUTOS RESERVA_TTL_MINUTOS RUST_LOG; do
            valor_clave="$(valor "$clave")"
            if [ -n "$valor_clave" ]; then
                printf '%s=%s\n' "$clave" "$valor_clave"
            fi
        done
    } > "$archivo"
    chmod 600 "$archivo"

    echo
    echo "Variables generadas en: $(pwd)/$archivo"
    if [ "$MODO" = "http" ]; then
        echo "ADVERTENCIA: el stack usará Caddyfile.http; el tráfico no estará cifrado."
        echo "Restrinja el puerto 80 a la red institucional o a una VPN."
    fi
    echo
    echo "Antes de desplegar, construya la imagen de la API en este host:"
    echo "  docker compose -f docker-compose.yml build api"
    echo
    echo "Despliegue desde Portainer:"
    echo "  1. Stacks -> Add stack -> Repository"
    echo "  2. Repositorio y rama; Compose path: infra/docker-compose.portainer.yml"
    echo "  3. Nombre del stack: expedientes"
    echo "  4. Environment variables -> cargue $archivo (o copie sus claves)"
    echo "  5. Deploy the stack"
    echo
    echo "El stack usa la imagen local $api_image y rutas absolutas del host."
    echo "Actualice este clon (git pull) antes de cambiar Caddyfile o certs."
    echo "Si el stack ya existe: Stacks -> expedientes -> Update the stack."
    echo "Después verifique el TLS desde el VPS con: bash ./verificar-tls.sh"
    echo "Elimine $archivo cuando Portainer ya tenga las variables: rm $archivo"
    echo
    echo "Cree la cuenta administradora desde la consola del contenedor api en Portainer:"
    echo "  env ADMIN_CORREO='jefe@institucion.mx' \\"
    echo "    ADMIN_PASSWORD='UnaClaveLargaYUnica#2026' \\"
    echo "    ADMIN_NOMBRE='Dirección jurídica' /usr/local/bin/crear_admin"
    exit 0
fi

docker compose up -d --build

echo "Esperando a que responda $ESQUEMA://$SITIO/health ..."
listo=0
for _ in $(seq 1 45); do
    # Forzar conexión local evita falsos negativos si el proveedor no permite
    # hairpin NAT desde el VPS hacia su propia IP pública.
    if [ "$MODO" = "tls" ]; then
        comprobacion=(curl -fsS --max-time 3 --resolve "$SITIO:443:127.0.0.1" "https://$SITIO/health")
    else
        comprobacion=(curl -fsS --max-time 3 --resolve "$SITIO:80:127.0.0.1" "http://$SITIO/health")
    fi
    if "${comprobacion[@]}" >/dev/null 2>&1; then
        listo=1
        break
    fi
    sleep 2
done

if [ "$listo" -ne 1 ]; then
    echo "La API no respondió todavía. Últimos registros de Caddy:" >&2
    docker compose logs --tail=40 caddy >&2 || true
    echo >&2
    echo "Diagnóstico TLS:" >&2
    echo "  bash ./verificar-tls.sh --sitio '$SITIO'" >&2
    echo "  docker compose ps" >&2
    echo "  docker compose logs --tail=80 caddy" >&2
    echo "  ss -tlnp | grep -E ':80|:443'" >&2
    echo "  getent hosts '$SITIO'" >&2
    echo "Confirme DNS y reglas TCP 80/443 en ufw y en el firewall del proveedor." >&2
    echo "Si cambió CADDYFILE en .env, recree Caddy: docker compose up -d --force-recreate caddy" >&2
    echo "No use --http en una red pública: transmite credenciales y expedientes sin cifrar." >&2
    exit 1
fi

echo "API disponible en $ESQUEMA://$SITIO"
echo "Verifique con: curl -fsS $ESQUEMA://$SITIO/health"
if [ "$MODO" = "tls" ]; then
    echo "Diagnóstico completo: bash ./verificar-tls.sh"
fi

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
