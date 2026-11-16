# Despliegue institucional

## Requisitos

- Servidor Debian 12 con Docker Engine y el plugin `docker compose`.
- DNS interno que resuelva el nombre elegido (por ejemplo `almacen.institucion.mx`).
- Puertos 80 y 443 abiertos solo en la LAN institucional o VPN.
- Reloj sincronizado (NTP): los tokens y la auditoría dependen de la hora.
- Certificado emitido por la CA institucional, o la CA interna de Caddy para pruebas.

## Preparación

```bash
sudo mkdir -p /opt/expedientes
sudo chown "$USER" /opt/expedientes
git clone <repositorio> /opt/expedientes
cd /opt/expedientes/infra

cp .env.example .env
# Generar secretos fuertes
openssl rand -hex 32   # JWT_SECRET
openssl rand -base64 24 # POSTGRES_PASSWORD
chmod 600 .env
```

Edite `.env` con `SITIO`, credenciales y rutas de certificados. **Nunca** versione `.env`.

### Certificados

- Institucional: copie `tls.crt` y `tls.key` en `infra/certs/` (ver `certs/LEEME.md`).
- Pruebas en LAN: use `Caddyfile.interno` (CA interna) y distribuya la raíz a los clientes.

```bash
# Variante con CA interna
cp Caddyfile Caddyfile.institucional
cp Caddyfile.interno Caddyfile
```

## Arranque

```bash
cd /opt/expedientes/infra
docker compose up -d --build
docker compose ps
curl --cacert certs/raiz-institucional.crt https://$SITIO/health
```

Solo Caddy publica puertos. PostgreSQL y la API quedan en la red interna de contenedores.

## Cuentas iniciales

```bash
# Solo para la puesta en marcha; crea cuentas y expedientes ficticios.
docker compose exec api /usr/local/bin/seed_demo
```

Las cuentas de demostración usan una contraseña conocida y `debe_cambiar_password = true`.
Cámbielas desde la aplicación en el primer acceso y elimine los expedientes `DEMO-*` si no los
necesita. Para una instalación real, cree el administrador con el rol `ADMINISTRADOR` y asigne
los demás usuarios desde el panel de administración.

## Clientes

1. Instale el paquete `Custodia Jurídica` (NSIS) generado con `npm run tauri build`.
2. Instale la CA institucional en el almacén de certificados de Windows.
3. En la pantalla de acceso, configure la dirección `https://almacen.institucion.mx`.
4. Verifique el estado `En línea` y realice una búsqueda de expedientes.

La dirección del servidor se guarda en la base local del equipo, no en el instalador.

## Actualización

```bash
cd /opt/expedientes
git pull
cd infra
./backup/backup.sh
docker compose up -d --build
```

Las migraciones se aplican al iniciar la API. Si una migración falla, la API no arranca y el
`restore/restore.sh` permite volver al respaldo previo.

## Operación sin internet

El servidor no requiere internet público. Los clientes solo necesitan alcanzar `SITIO` por LAN o
VPN. Si el servidor no está disponible, los clientes conservan su caché y el outbox; las
confirmaciones de custodia, entrega, devolución y permisos quedan bloqueadas hasta reconectar.

## Verificación posterior

- `GET /health` responde `{"ok":true}` a través de TLS.
- Un usuario sin permisos recibe 403.
- La bitácora `auditoria` registra el acceso.
- `./backup/backup.sh` produce un `.dump` y la restauración se prueba en un entorno separado.
