#!/usr/bin/env bash
# Diagnóstico del acceso TLS de Caddy, útil en VPS administrados por Portainer.
# No reinicia contenedores ni modifica configuración.
# Uso:
#   ./verificar-tls.sh
#   ./verificar-tls.sh --sitio 129-121-117-177.sslip.io
#   ./verificar-tls.sh --proyecto expedientes --sin-externo
set -euo pipefail

cd "$(dirname "$0")"

SITIO_ARG=""
PROYECTO="${COMPOSE_PROJECT_NAME:-}"
EXTERNO=1

uso() {
    cat <<'EOF'
Uso: ./verificar-tls.sh [opciones]

  --sitio DOMINIO     Dominio público (por omisión, el del contenedor Caddy)
  --proyecto NOMBRE   Proyecto de Compose/Portainer si hay varios stacks
  --sin-externo       Omite la prueba contra la IP pública (útil sin hairpin NAT)
  -h, --help          Esta ayuda

Comprueba el archivo de Caddy montado, la configuración efectiva, el puerto 443,
el saludo TLS y el certificado. Usa Docker directamente, sin exigir el plugin
`docker compose`, y lee el SITIO efectivo del contenedor.
Salidas: 0 TLS correcto; 1 configuración/TLS local; 2 acceso externo; 3 modo HTTP.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --sitio) SITIO_ARG="${2:?falta el dominio}"; shift 2 ;;
        --proyecto) PROYECTO="${2:?falta el nombre del proyecto}"; shift 2 ;;
        --sin-externo) EXTERNO=0; shift ;;
        -h | --help) uso; exit 0 ;;
        *) echo "Opción desconocida: $1" >&2; uso; exit 2 ;;
    esac
done

for herramienta in docker curl; do
    command -v "$herramienta" >/dev/null 2>&1 || {
        echo "Falta $herramienta en el sistema" >&2
        exit 1
    }
done

filtros=(--filter label=com.docker.compose.service=caddy --filter status=running)
if [ -n "$PROYECTO" ]; then
    filtros+=(--filter "label=com.docker.compose.project=$PROYECTO")
fi

mapfile -t ids_caddy < <(docker ps -q "${filtros[@]}")
if [ "${#ids_caddy[@]}" -eq 0 ]; then
    echo "ERROR: no se encontró un contenedor Caddy en ejecución." >&2
    echo "Si el stack tiene otro nombre, ejecute: docker ps --format '{{.Names}} {{.Labels}}'" >&2
    echo "y repita con --proyecto NOMBRE." >&2
    exit 1
fi
if [ "${#ids_caddy[@]}" -gt 1 ]; then
    echo "ERROR: hay varios contenedores Caddy en ejecución." >&2
    docker ps --filter label=com.docker.compose.service=caddy \
        --format '  {{.Names}} (proyecto {{.Label "com.docker.compose.project"}})' >&2
    echo "Repita con --proyecto NOMBRE." >&2
    exit 1
fi
caddy_id="${ids_caddy[0]}"

echo "Contenedor Caddy: $(docker inspect "$caddy_id" --format '{{.Name}}' | sed 's#^/##')"
echo

echo "== Archivo de Caddy montado =="
origen="$(docker inspect "$caddy_id" \
    --format '{{range .Mounts}}{{if eq .Destination "/etc/caddy/Caddyfile"}}{{.Source}}{{end}}{{end}}' \
    2>/dev/null || true)"
echo "Origen: ${origen:-no detectado}"

config_incorrecta=0
variante="desconocida"
case "$origen" in
    *Caddyfile.publico)
        variante="publico"
        echo "Variante: Let's Encrypt (correcta para un VPS público)"
        ;;
    *Caddyfile.interno)
        variante="interno"
        echo "Variante: CA interna; los clientes deben confiar en esa CA"
        ;;
    *Caddyfile.http)
        variante="http"
        echo "ERROR: el contenedor está montado con Caddyfile.http (sin TLS)."
        config_incorrecta=1
        ;;
    *Caddyfile)
        variante="institucional"
        echo "Variante: certificados institucionales en certs/"
        ;;
    "")
        echo "ERROR: no se pudo leer el origen del montaje."
        config_incorrecta=1
        ;;
    *)
        echo "ADVERTENCIA: archivo de configuración no reconocido: $origen"
        ;;
esac

if [ -n "$origen" ] && [ ! -f "$origen" ]; then
    echo "ERROR: la ruta montada no existe en el host. Si Portainer recreó el clon"
    echo "del repositorio, el montaje quedó huérfano; use rutas absolutas estables."
    config_incorrecta=1
fi
echo

# SITIO efectivo: argumento, variable del contenedor o .env como último recurso.
SITIO="$SITIO_ARG"
if [ -z "$SITIO" ]; then
    SITIO="$(docker exec "$caddy_id" printenv SITIO 2>/dev/null | tail -1 || true)"
fi
if [ -z "$SITIO" ]; then
    SITIO="$(docker inspect "$caddy_id" \
        --format '{{range .Config.Env}}{{println .}}{{end}}' 2>/dev/null \
        | sed -n 's/^SITIO=//p' | tail -1 || true)"
fi
if [ -z "$SITIO" ] && [ -f .env ]; then
    SITIO="$(sed -n 's/^SITIO=//p' .env | tail -1)"
fi
SITIO="${SITIO#http://}"
SITIO="${SITIO#https://}"
SITIO="${SITIO%%/*}"
if [ -z "$SITIO" ]; then
    echo "ERROR: no se pudo determinar el dominio. Use --sitio o defina SITIO." >&2
    exit 1
fi
echo "Sitio efectivo: $SITIO"
echo

echo "== Configuración efectiva de Caddy =="
config_efectiva="$(docker exec "$caddy_id" cat /etc/caddy/Caddyfile 2>/dev/null || true)"
if [ -z "$config_efectiva" ]; then
    echo "ERROR: no se pudo leer /etc/caddy/Caddyfile dentro del contenedor." >&2
    config_incorrecta=1
else
    printf '%s\n' "$config_efectiva" | sed -n '1,25p'
    if printf '%s\n' "$config_efectiva" | grep -q 'auto_https off'; then
        echo
        echo "ERROR: la configuración tiene 'auto_https off'."
        config_incorrecta=1
    fi
    if printf '%s\n' "$config_efectiva" | grep -qE '^[[:space:]]*:80[[:space:]]*\{'; then
        echo
        echo "ERROR: el sitio escucha solo en :80; no hay TLS configurado."
        config_incorrecta=1
    fi
fi
echo

# Dirección local donde Docker publicó 443; si es 0.0.0.0 usamos loopback.
bind443="$(docker port "$caddy_id" 443/tcp 2>/dev/null | head -1 || true)"
host_443="127.0.0.1"
if [ -n "$bind443" ]; then
    case "$bind443" in
        \[*\]:*) host_443="127.0.0.1" ;;
        *:*) host_443="${bind443%:*}" ;;
    esac
    if [ "$host_443" = "0.0.0.0" ]; then
        host_443="127.0.0.1"
    fi
fi

echo "== Puerto 443 publicado =="
if [ -n "$bind443" ]; then
    echo "Caddy publica 443 en: $bind443 (prueba local contra $host_443)"
else
    echo "ERROR: Caddy no publica 443; revise 'ports' y BIND_IP."
    config_incorrecta=1
fi
echo

if [ "$variante" = "http" ]; then
    echo "== Resultado =="
    echo "El stack está en modo HTTP sin cifrado; no hay TLS que verificar."
    echo "Para TLS público use CADDYFILE=./Caddyfile.publico y recree Caddy:"
    echo "  docker compose up -d --force-recreate caddy"
    echo "En Portainer: actualice CADDYFILE en el stack y use Update the stack."
    exit 3
fi

echo "== Saludo TLS local =="
codigo_tls="$(curl -k -s -o /dev/null -w '%{http_code}' --max-time 8 \
    --resolve "$SITIO:443:$host_443" "https://$SITIO/health" 2>/dev/null || true)"
tls_ok=0
case "$codigo_tls" in
    "" | 000)
        echo "FALLÓ: no se completó el saludo TLS contra $host_443:443."
        ;;
    *)
        tls_ok=1
        echo "OK: TLS completado (HTTP $codigo_tls)."
        if [ "$codigo_tls" != "200" ]; then
            echo "La API o la base pueden estar degradadas; el certificado sí se presentó."
        fi
        ;;
esac

confianza=0
huella_local=""
salida_ssl=""
if command -v openssl >/dev/null 2>&1; then
    echo
    echo "== Certificado presentado =="
    if command -v timeout >/dev/null 2>&1; then
        salida_ssl="$(timeout 8 openssl s_client -connect "${host_443}:443" \
            -servername "$SITIO" </dev/null 2>&1 || true)"
    else
        salida_ssl="$(openssl s_client -connect "${host_443}:443" \
            -servername "$SITIO" </dev/null 2>&1 || true)"
    fi

    certificado="$(printf '%s\n' "$salida_ssl" | openssl x509 -noout \
        -subject -issuer -dates -fingerprint -sha256 2>/dev/null || true)"
    if [ -n "$certificado" ]; then
        printf '%s\n' "$certificado"
        huella_local="$(printf '%s\n' "$salida_ssl" | openssl x509 -noout \
            -fingerprint -sha256 2>/dev/null || true)"
    else
        echo "No se pudo leer un certificado (el saludo TLS no se completó)."
    fi

    if printf '%s\n' "$salida_ssl" | grep -q 'Verify return code: 0 (ok)'; then
        confianza=1
        echo "Cadena verificada con las CA de este host."
    else
        echo "ADVERTENCIA: la cadena no se pudo verificar con las CA de este host."
        if [ "$variante" = "interno" ] || [ "$variante" = "institucional" ]; then
            echo "Es esperable con CA interna o institucional: instálela en el cliente."
        fi
    fi
fi

echo
echo "== Respuesta por HTTP (puerto 80) =="
bind80="$(docker port "$caddy_id" 80/tcp 2>/dev/null | head -1 || true)"
host_80="127.0.0.1"
if [ -n "$bind80" ]; then
    case "$bind80" in
        \[*\]:*) host_80="127.0.0.1" ;;
        *:*) host_80="${bind80%:*}" ;;
    esac
    if [ "$host_80" = "0.0.0.0" ]; then
        host_80="127.0.0.1"
    fi
fi
codigo_http="$(curl -s -o /dev/null -w '%{http_code}' --max-time 8 \
    --resolve "$SITIO:80:$host_80" "http://$SITIO/health" 2>/dev/null || true)"
case "$codigo_http" in
    301 | 302 | 307 | 308)
        echo "OK: HTTP redirige a HTTPS ($codigo_http)."
        ;;
    200)
        echo "ERROR: HTTP responde 200; Caddy está sirviendo en claro."
        config_incorrecta=1
        ;;
    "")
        echo "Sin respuesta por HTTP (aceptable si solo publica 443)."
        ;;
    *)
        echo "Código HTTP: $codigo_http"
        ;;
esac

externo_ok=1
if [ "$EXTERNO" -eq 1 ]; then
    echo
    echo "== Acceso externo =="
    codigo_ext="$(curl -k -s -o /dev/null -w '%{http_code}' --max-time 8 \
        "https://$SITIO/health" 2>/dev/null || true)"
    case "$codigo_ext" in
        "" | 000)
            externo_ok=0
            echo "FALLÓ: no se completó TLS contra $SITIO:443 desde este host."
            ;;
        *)
            echo "OK: TLS externo completado (HTTP $codigo_ext)."
            ;;
    esac

    if [ "$externo_ok" -eq 1 ]; then
        if curl -sS -o /dev/null --max-time 8 "https://$SITIO/health" 2>/dev/null; then
            echo "Certificado externo verificado con las CA de este host."
        else
            echo "ADVERTENCIA: el certificado externo no se pudo verificar con las CA del host."
        fi

        if command -v openssl >/dev/null 2>&1; then
            if command -v timeout >/dev/null 2>&1; then
                salida_ext="$(timeout 8 openssl s_client -connect "${SITIO}:443" \
                    -servername "$SITIO" </dev/null 2>&1 || true)"
            else
                salida_ext="$(openssl s_client -connect "${SITIO}:443" \
                    -servername "$SITIO" </dev/null 2>&1 || true)"
            fi
            huella_ext="$(printf '%s\n' "$salida_ext" | openssl x509 -noout \
                -fingerprint -sha256 2>/dev/null || true)"
            if [ -n "$huella_local" ] && [ -n "$huella_ext" ] && [ "$huella_local" != "$huella_ext" ]; then
                echo "ERROR: el certificado externo no coincide con el local."
                echo "Revise DNS, proxy delante del VPS o balanceadores."
                externo_ok=0
            fi
        fi
    fi
fi

if command -v ss >/dev/null 2>&1; then
    if ss -tln 2>/dev/null | grep -qE '[:.]8000[[:space:]]'; then
        echo
        echo "ADVERTENCIA: el puerto 8000 (Portainer Edge) está publicado. Si no usa"
        echo "Edge, elimínelo del stack de Portainer o restríngalo en el firewall."
    fi
    if ss -tln 2>/dev/null | grep -qE '[:.]9443[[:space:]]'; then
        echo
        echo "ADVERTENCIA: 9443 (UI de Portainer) está publicado. Publíquelo solo en"
        echo "127.0.0.1 o en la VPN y acceda por túnel SSH."
    fi
fi

if [ "$tls_ok" -eq 0 ]; then
    echo
    echo "== Últimos registros de Caddy =="
    docker logs --tail=30 "$caddy_id" 2>/dev/null || true
fi

echo
echo "== Resumen =="
if [ "$tls_ok" -eq 1 ]; then
    echo "TLS local: OK"
else
    echo "TLS local: FALLÓ"
fi
if [ "$confianza" -eq 1 ]; then
    echo "Confianza local: OK"
elif command -v openssl >/dev/null 2>&1; then
    echo "Confianza local: no verificada"
fi
if [ "$EXTERNO" -eq 1 ]; then
    if [ "$externo_ok" -eq 1 ]; then
        echo "Acceso externo: OK"
    else
        echo "Acceso externo: FALLÓ"
    fi
fi

if [ "$config_incorrecta" -eq 1 ] || [ "$tls_ok" -eq 0 ]; then
    echo
    echo "Sugerencia: aplique la variante TLS y recree solo Caddy:"
    echo "  docker compose up -d --force-recreate caddy"
    echo "En Portainer: actualice CADDYFILE/CERTS_DIR y use Update the stack."
    exit 1
fi

if [ "$variante" = "publico" ] && command -v openssl >/dev/null 2>&1 && [ "$confianza" -eq 0 ]; then
    echo
    echo "ERROR: el certificado público no es confiable para este host."
    echo "Revise los registros de Caddy y la emisión de Let's Encrypt."
    exit 1
fi

if [ "$EXTERNO" -eq 1 ] && [ "$externo_ok" -eq 0 ]; then
    echo
    echo "El TLS local funciona, pero el acceso externo no. Revise el firewall del VPS,"
    echo "las reglas del proveedor para TCP 80/443 y el DNS de $SITIO."
    echo "Si el proveedor no permite hairpin NAT, repita con --sin-externo."
    exit 2
fi

echo
echo "TLS operativo."
