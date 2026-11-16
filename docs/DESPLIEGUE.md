# Despliegue institucional

## Requisitos

- Servidor Debian 12 con Docker Engine y el plugin `docker compose`.
- DNS interno que resuelva el nombre elegido (por ejemplo `almacen.institucion.mx`).
- Puertos 80 y 443 abiertos solo en la LAN institucional o VPN.
- Reloj sincronizado (NTP): los tokens y la auditoría dependen de la hora.
- Certificado emitido por la CA institucional, o la CA interna de Caddy para pruebas.

## Ruta rápida para VPS con dominio público

Resumen: DNS → SSH → firewall → Docker → configuración → TLS automático → arranque →
administrador → respaldos.

### 1. DNS

Cree un registro `A` (y `AAAA` si el VPS tiene IPv6) para el dominio elegido apuntando a la IP
del VPS. Espere la propagación antes de arrancar Caddy; de lo contrario falla la emisión del
certificado.

Si usa Cloudflare, deje el registro en modo **DNS only** (nube gris) durante la emisión.

### 2. Endurecer el acceso

```bash
# Como root, una sola vez
adduser --gecos "" deploy
usermod -aG sudo deploy
rsync --archive --chown=deploy:deploy ~/.ssh /home/deploy

# /etc/ssh/sshd_config
#   PermitRootLogin no
#   PasswordAuthentication no
systemctl restart ssh
```

A partir de aquí trabaje como `deploy`.

### 3. Firewall

```bash
sudo apt update && sudo apt install -y ufw fail2ban
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

No abra 3000 ni 5432: no están publicados por el Compose y no deben estarlo.

### 4. Docker

```bash
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/debian/gpg -o /tmp/docker.asc
sudo install -m 0644 /tmp/docker.asc /etc/apt/keyrings/docker.asc

echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian $(. /etc/os-release && echo $VERSION_CODENAME) stable" \
  | sudo tee /etc/apt/sources.list.d/docker.list

sudo apt update
sudo apt install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
sudo usermod -aG docker deploy
newgrp docker
```

Dimensionamiento sugerido: 2 vCPU, 4 GB de RAM y 40 GB de disco. La compilación de la imagen
consume memoria; con 2 GB agregue swap antes de construir:

```bash
sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile
sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

### 5. Desplegar

```bash
sudo mkdir -p /opt/expedientes && sudo chown deploy:deploy /opt/expedientes
git clone <repositorio> /opt/expedientes
cd /opt/expedientes/infra

cp .env.example .env
chmod 600 .env

# Genere secretos y péguelos en .env
openssl rand -hex 32      # JWT_SECRET
openssl rand -base64 24   # POSTGRES_PASSWORD

# Edite SITIO y ACME_EMAIL
nano .env

# TLS automático con Let's Encrypt
cp Caddyfile.publico Caddyfile

docker compose up -d --build
docker compose ps
docker compose logs -f caddy   # espere "certificate obtained successfully"
```

### 6. Crear el administrador

No use `seed_demo` en un VPS expuesto a internet. Cree solo la cuenta administradora:

```bash
docker compose exec api env \
  ADMIN_CORREO='jefe@institucion.mx' \
  ADMIN_PASSWORD='UnaClaveLargaYUnica#2026' \
  ADMIN_NOMBRE='Dirección jurídica' \
  /usr/local/bin/crear_admin
```

El comando es idempotente en el sentido de que rechaza correos duplicados; no crea datos de
demostración. Después cree el resto de usuarios desde el panel de Administración.

### 7. Comprobar

```bash
curl -fsS https://$SITIO/health
curl -sI https://$SITIO | grep -i strict-transport-security
docker compose logs --tail=50 api
```

Inicie sesión desde la aplicación con la URL `https://<dominio>`; con certificado público no hace
falta instalar ninguna CA en los clientes.

### 8. Respaldos y actualizaciones

Programe el respaldo y cópielo fuera del VPS (almacenamiento institucional, `rclone` o `scp`):

```cron
0 2,14 * * * cd /opt/expedientes/infra && ./backup/backup.sh >> backup/registro.log 2>&1
```

Actualizar:

```bash
cd /opt/expedientes && git pull
cd infra && ./backup/backup.sh && docker compose up -d --build
```

### 9. Advertencia de datos

Los expedientes jurídicos son información sensible. Antes de alojarlos en un VPS confirme que la
institución autoriza esa ubicación, que el disco está cifrado, que los respaldos se guardan
cifrados y que el acceso SSH está restringido. Si la normativa exige residencia interna, use el
servidor Debian institucional en lugar de un VPS público.

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
- VPS con dominio público: use `Caddyfile.publico` (Let's Encrypt automático).
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
