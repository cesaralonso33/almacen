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

Docker publica los puertos en sus propias cadenas de iptables; **UFW no filtra** los puertos de un
contenedor. Si la instalación debe quedar solo en LAN/VPN, fije `BIND_IP` a la IP institucional del
servidor en `infra/.env`; así Docker no publica 80/443 en todas las interfaces. Para los puertos de
Portainer, use loopback, VPN o la cadena `DOCKER-USER`/firewall del proveedor.

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

**Opción en un paso (recomendada).** El script genera secretos, elige `sslip.io` + Let's Encrypt,
levanta el stack y verifica `/health`:

```bash
sudo mkdir -p /opt/expedientes && sudo chown deploy:deploy /opt/expedientes
git clone <repositorio> /opt/expedientes
cd /opt/expedientes/infra
chmod +x preparar-vps.sh

./preparar-vps.sh \
  --admin-correo 'jefe@institucion.mx' \
  --admin-password 'UnaClaveLargaYUnica#2026' \
  --admin-nombre 'Dirección jurídica'
```

Si el proveedor bloquea 80/443 y no hay TLS posible:

```bash
./preparar-vps.sh --http
```

Opciones: `--sitio <dominio>` si ya tiene uno, `--http` para red cerrada, `--admin-*` para crear la
cuenta inicial.

**Opción manual**, si prefiere controlar cada paso:

```bash
sudo mkdir -p /opt/expedientes && sudo chown deploy:deploy /opt/expedientes
git clone <repositorio> /opt/expedientes
cd /opt/expedientes/infra

cp .env.example .env
chmod 600 .env

# Genere secretos y péguelos en .env
openssl rand -hex 32      # JWT_SECRET
openssl rand -hex 24      # POSTGRES_PASSWORD

# Edite SITIO y CADDYFILE DENTRO de .env (no solo en la shell)
sed -i 's|^SITIO=.*|SITIO=129-121-117-177.sslip.io|' .env
sed -i 's|^CADDYFILE=.*|CADDYFILE=./Caddyfile.publico|' .env
grep -E '^(SITIO|CADDYFILE)=' .env

docker compose up -d --build
docker compose ps
docker compose logs -f caddy   # espere "certificate obtained successfully"
```

La opción manual debe dejar `CADDYFILE=./Caddyfile.publico` **grabado en `.env`**; una asignación
suelta en la terminal no llega a `docker compose`. El valor de ejemplo `./Caddyfile` espera
certificados institucionales en `infra/certs/`. Publicar el puerto 443 no basta: Caddy también
debe montar el archivo TLS correcto. Si cambia `CADDYFILE` después de crear el contenedor, recree
Caddy con `docker compose up -d --force-recreate caddy`.

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
set -a; . ./.env; set +a
curl -fsS "https://$SITIO/health"
curl -sI https://$SITIO | grep -i strict-transport-security
docker compose logs --tail=50 api
```

Inicie sesión desde la aplicación con la URL `https://<dominio>`; con certificado público no hace
falta instalar ninguna CA en los clientes.

### 8. Respaldos y actualizaciones

Programe el respaldo y cópielo fuera del VPS (almacenamiento institucional, `rclone` o `scp`):

```cron
0 2,14 * * * cd /opt/expedientes/infra && bash ./backup/backup.sh >> backup/registro.log 2>&1
```

Actualizar:

```bash
cd /opt/expedientes && git pull
cd infra && bash ./backup/backup.sh && docker compose up -d --build
```

### 9. Advertencia de datos

Los expedientes jurídicos son información sensible. Antes de alojarlos en un VPS confirme que la
institución autoriza esa ubicación, que el disco está cifrado, que los respaldos se guardan
cifrados y que el acceso SSH está restringido. Si la normativa exige residencia interna, use el
servidor Debian institucional en lugar de un VPS público.

### 10. Sin dominio: usar solo la IP

Es posible, pero **Let's Encrypt no emite certificados para direcciones IP** de forma general, así
que `Caddyfile.publico` no sirve. El camino correcto es la CA interna de Caddy:

1. En `infra/.env`:

   ```env
   SITIO=203.0.113.10
   ```

2. Indique la variante con CA interna en `.env`:

   ```env
   CADDYFILE=./Caddyfile.interno
   ```

   ```bash
   docker compose up -d --build
   ```

3. Extraiga la raíz de Caddy y distribúyala a los clientes:

   ```bash
   docker compose exec caddy cat /data/caddy/pki/authorities/local/root.crt > raiz-interna.crt
   ```

4. Instale `raiz-interna.crt` en cada equipo Windows:
   doble clic → **Instalar certificado** → **Equipo local** → *Entidades de certificación raíz de
   confianza*. En dominio, distribúyala por GPO.

5. Compruebe que el certificado incluye la IP como SAN:

   ```bash
   openssl s_client -connect 203.0.113.10:443 -servername 203.0.113.10 </dev/null 2>/dev/null \
     | openssl x509 -noout -subject -ext subjectAltName
   curl --cacert raiz-interna.crt https://203.0.113.10/health
   ```

6. En la aplicación, configure `https://203.0.113.10`.

El cliente de escritorio usa el almacén de certificados del sistema (`rustls-tls-native-roots`),
por lo que confía en la CA una vez instalada.

Consideraciones:

- La IP debe ser **fija**; si cambia, hay que volver a emitir y distribuir.
- HSTS no aplica a direcciones IP, pero el tráfico sigue cifrado.
- Para producción institucional es preferible un nombre DNS (aunque sea interno) y un certificado
  de la CA institucional; la IP es aceptable para pruebas o redes cerradas.

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

La variante se elige con `CADDYFILE` en `.env`; no hace falta copiar archivos:

```env
CADDYFILE=./Caddyfile.publico   # dominio público, Let's Encrypt
CADDYFILE=./Caddyfile.interno   # solo IP o LAN, CA interna
CADDYFILE=./Caddyfile           # certificados institucionales en certs/
```

Estas rutas relativas son para el despliegue por CLI. En el stack de Portainer deben ser
**absolutas** (`/opt/.../infra/Caddyfile.publico` y `/opt/.../infra/certs`);
`preparar-vps.sh --portainer` las genera así.

## Sin TLS (solo HTTP, red cerrada)

Existe una variante sin cifrado para cuando no es posible configurar certificados:

```env
# infra/.env
SITIO=129.121.117.177
CADDYFILE=./Caddyfile.http
```

```bash
docker compose up -d --force-recreate caddy
docker exec expedientes-caddy-1 head -5 /etc/caddy/Caddyfile   # debe decir auto_https off
curl -v http://129.121.117.177/health
```

En la aplicación use `http://129.121.117.177`. La pantalla de acceso y la barra superior muestran
**«Sin cifrado»**.

### Riesgo y mitigación obligatoria

Con HTTP, las credenciales, los tokens y los datos de expedientes viajan **en claro**. Solo es
aceptable si el puerto 80 está restringido a la red institucional o a una VPN:

```bash
# Ejemplo: permitir solo la red institucional y cerrar al resto
sudo ufw delete allow 80/tcp
sudo ufw allow from 10.0.0.0/8 to any port 80 proto tcp
sudo ufw allow from 192.168.0.0/16 to any port 80 proto tcp
sudo ufw status numbered
```

Alternativas recomendadas, en orden:

1. Dominio público o `sslip.io` con Let's Encrypt (`Caddyfile.publico`).
2. VPN (WireGuard/Tailscale) que cifre el transporte aunque el servicio sea HTTP.
3. Certificado institucional (`certs/` + `Caddyfile`).

Planifique migrar a TLS: el modo HTTP es una contingencia, no un estado final.

## Coexistencia con Portainer

Portainer usa por omisión **9443** (interfaz), **8000** (Edge) y opcionalmente **9000**; este
stack solo necesita **80** y **443**. Compruebe quién tiene los puertos:

```bash
sudo ss -tlnp | grep -E ':80|:443|:8000|:9443'
docker ps --format 'table {{.Names}}\t{{.Ports}}'
```

Si Portainer está publicado en 443, cámbielo a `127.0.0.1:9443:9443` y acceda por túnel SSH; así
Caddy puede quedarse con 80/443.

**Los puertos publicados por Docker no pasan por las reglas de UFW.** Docker crea sus propias
cadenas de iptables, así que `ufw deny 8000` no cierra un puerto publicado con `-p`. Para los
puertos de administración:

- **9443 (UI de Portainer):** publíquelo solo en `127.0.0.1` en el Compose de Portainer
  (`127.0.0.1:9443:9443`) y acceda por túnel SSH:
  `ssh -L 9443:127.0.0.1:9443 deploy@VPS`, luego `https://127.0.0.1:9443`.
- **8000 (Edge):** elimínelo del Compose de Portainer si no usa Edge. Si lo usa, restrínjalo en el
  firewall del proveedor o en la cadena `DOCKER-USER`.
- **80/443:** deben ser alcanzables para Let's Encrypt y los clientes. Si la instalación es solo
  LAN/VPN, fije `BIND_IP` a la IP institucional del servidor; así Docker no los publica en todas
  las interfaces.

No abra 3000 ni 5432: la API y PostgreSQL no se publican y no deben hacerlo.

### Preparar imagen y variables

Los stacks Git de Portainer clonan el repositorio dentro de su contenedor. Por eso **no son
fiables** los montajes relativos (`./Caddyfile`, `./certs`) ni la compilación de la imagen desde
`../server`. Este proyecto usa un stack específico con:

- imagen de API ya construida en el host;
- rutas **absolutas** para el Caddyfile y los certificados.

```bash
cd /opt/expedientes/almacen/infra     # ajuste la ruta de su clon
git pull
docker compose -f docker-compose.yml build api
chmod +x preparar-vps.sh verificar-tls.sh
./preparar-vps.sh --portainer --sitio 129-121-117-177.sslip.io
```

El script crea `infra/portainer.env` con permisos `600`, sin levantar contenedores. Incluye
`API_IMAGE`, `CADDYFILE` y `CERTS_DIR` con rutas absolutas, además de secretos y `SITIO`. No lo
versione ni lo deje en el servidor después de cargarlo en Portainer.

### Crear o actualizar el stack

1. *Stacks* → *Add stack* → **Repository**.
2. Repositorio y rama; **Compose path**: `infra/docker-compose.portainer.yml`.
3. **Nombre del stack**: `expedientes` (así coincide con el proyecto de `docker compose`).
4. *Environment variables* → cargue `portainer.env` o copie sus claves.
5. *Deploy the stack*.

Portainer no compila la API en este flujo: usa la imagen local construida en el paso anterior. Si
la imagen no existe en el host, el servicio `api` no arranca; construya y etiquete la imagen antes
de desplegar. Este flujo supone que Portainer administra el Docker del **mismo VPS**; con un
entorno remoto, publique `API_IMAGE` en un registro accesible desde ese host.

Mantenga actualizado el clon del host (`git pull`) antes de *Update the stack*: las rutas absolutas
de Caddy apuntan a ese clon. Un cambio de `CADDYFILE` o `CERTS_DIR` no se aplica hasta recrear
Caddy. Este flujo está pendiente de verificación en el VPS.

### Verificar TLS

Desde el VPS, en `infra`:

```bash
bash ./verificar-tls.sh
```

El diagnóstico lee el `SITIO` efectivo del contenedor, revisa el archivo montado, la configuración
efectiva, el puerto 443, el saludo TLS y el certificado. Usa Docker directamente, sin exigir el
plugin `docker compose`.

Salidas: `0` TLS correcto; `1` configuración o TLS local; `2` acceso externo; `3` modo HTTP sin
cifrado. Si informa `auto_https off` o un sitio `:80`, el contenedor conserva una configuración
anterior (por ejemplo, `Caddyfile.http`). Corrija el valor en Portainer y use *Update the stack*;
como alternativa temporal desde el host:

```bash
docker compose up -d --force-recreate caddy
```

Use `--sin-externo` si el proveedor no permite hairpin NAT desde el VPS hacia su IP pública.
Si el stack no se llama `expedientes`, agregue `--proyecto NOMBRE`.

### Operación con Portainer

- Los comandos `docker compose` de `infra` operan sobre el mismo stack si se llama `expedientes`.
  Si usó otro nombre, agregue `-p NOMBRE` o exporte `COMPOSE_PROJECT_NAME`.
- Cuando Portainer administra el stack, las variables efectivas son las de la UI, no las de
  `infra/.env`. Un comando por SSH puede corregir el contenedor de forma temporal, pero la
  siguiente actualización desde Portainer puede revertirlo.
- Programe los respaldos por SSH y cron (`backup/backup.sh`); no dependa del contenedor de
  Portainer para conservar copias.
- Antes de *Update the stack*, ejecute un respaldo. Las migraciones se aplican al iniciar la API y
  `restore/restore.sh` permite volver al respaldo previo si una falla.

## Arranque

```bash
cd /opt/expedientes/infra
docker compose up -d --build
docker compose ps
curl --cacert certs/raiz-institucional.crt "https://$(grep '^SITIO=' .env | cut -d= -f2)/health"
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

1. Instale el paquete `Custodia CradSystem` (NSIS) generado con `npm run tauri build`.
2. Instale la CA institucional en el almacén de certificados de Windows.
3. En la pantalla de acceso, la dirección viene precargada con `https://129-121-117-177.sslip.io`; cámbiela si su instalación usa otro nombre o HTTP.
4. Verifique el estado `En línea` y realice una búsqueda de expedientes.

La dirección del servidor se guarda en la base local del equipo, no en el instalador.

## Actualización

```bash
cd /opt/expedientes
git pull
cd infra
chmod +x backup/backup.sh
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
