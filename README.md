# Custodia CradSystem

Sistema para solicitud, entrega, devolución y cadena de custodia de expedientes jurídicos físicos.
Estado actual: **funcional, con autenticación, RBAC, operación offline e interfaz de escritorio**.
La API autentica con Argon2id + JWT, aplica permisos por rol, registra auditoría y ejecuta el ciclo
de pedidos y custodia. La aplicación de escritorio consume la API y mantiene caché local.

## Requisitos

- Rust estable (probado con 1.90)
- Node.js 20+ y npm
- PostgreSQL 14+ (probado con 14.5 local)
- Dependencias de Tauri 2 para el sistema operativo

## Base de datos y API

Con Docker Compose (recomendado para desarrollo):

```bash
cd infra
cp .env.example .env      # ajustar POSTGRES_PASSWORD y JWT_SECRET
docker compose up -d
export DATABASE_URL='postgres://expedientes:cambie_esta_clave_de_desarrollo@localhost:5432/expedientes'
```

O con un PostgreSQL local:

```bash
createdb expedientes
export DATABASE_URL='postgres://usuario:clave@localhost:5432/expedientes'
```

Iniciar la API (las migraciones se aplican al arrancar):

```bash
cd server
export JWT_SECRET='un_secreto_aleatorio_de_al_menos_32_caracteres'
cargo run                      # escucha en http://localhost:3000
cargo run --bin seed_demo      # opcional: cuentas y expedientes de demostración
```

Variables reconocidas: `DATABASE_URL`, `JWT_SECRET` (obligatorias), `ACCESS_TTL_MINUTOS` (15),
`REFRESH_TTL_MINUTOS` (10080), `RESERVA_TTL_MINUTOS` (30), `BIND_ADDR` (`0.0.0.0:3000`),
`RUST_LOG`. Ver `server/.env.example`.

`GET /health` comprueba la base de datos.

### Cuentas de demostración (`seed_demo`)

| Correo | Rol | Contraseña |
|---|---|---|
| `admin@demo.mx` | ADMINISTRADOR | `Demo#2026` |
| `almacen@demo.mx` | ALMACEN | `Demo#2026` |
| `solicitante@demo.mx` | USUARIO | `Demo#2026` |
| `consulta@demo.mx` | CONSULTA | `Demo#2026` |

Todas se crean con `debe_cambiar_password = true`; nunca deben usarse en producción.

### Pruebas

```bash
cd server
export TEST_ADMIN_DATABASE_URL='postgres://postgres:clave@localhost:5432/postgres'
cargo test
```

Cada prueba crea una base efímera, aplica todas las migraciones y la elimina. Cubre autenticación,
sesiones, RBAC, administración de usuarios, flujo de pedidos, jerarquía de ubicaciones, reservas,
descartes catalogados, auditoría append-only, sincronización, idempotencia y custodia única.

## Autenticación y autorización

- `POST /api/v1/auth/login` `{ "correo", "password" }` → tokens, roles y permisos.
- `POST /api/v1/auth/refresh` `{ "refresh_token" }` → rota el refresh (el anterior se revoca).
- `POST /api/v1/auth/logout` → revoca la sesión actual.
- `POST /api/v1/auth/password` `{ "actual", "nueva" }` → cambia la contraseña y revoca todas las sesiones.
- `GET /api/v1/auth/me` → identidad, roles y permisos.

Las rutas protegidas exigen `Authorization: Bearer <access_token>`. El token de acceso dura 15
minutos; el refresh es opaco, se guarda como hash SHA-256, dura 7 días y rota en cada uso.
Las contraseñas se almacenan con Argon2id. Los errores tienen la forma
`{"error": {"code", "message", "correlacion_id"}}`.

La identidad **nunca** se acepta desde el cuerpo: enviar `actor_id`, `solicitante_id` o
`receptor_id` produce 400.

## Endpoints de negocio

- `POST /api/v1/pedidos` `{ "expedientes": ["UUID"] }` — permiso `pedidos:crear`.
- `GET /api/v1/pedidos/{id}` — propietario o permiso `pedidos:preparar`.
- `POST /api/v1/items/{id}/preparar` — permiso `pedidos:preparar`; crea reserva con vencimiento.
- `POST /api/v1/items/{id}/descartar` `{ "motivo_clave", "detalle" }` — permiso `pedidos:descartar`.
- `POST /api/v1/entregas` `{ "item_ids": ["UUID"] }` — permiso `entregas:despachar`.
- `POST /api/v1/entregas/{id}/confirmar` — solo el receptor designado; genera el acuse.
- `POST /api/v1/pedidos/{id}/cancelar` `{ "motivo" }` — solo el solicitante, sin renglones entregados.
- `POST /api/v1/devoluciones` `{ "expediente_id", "observaciones" }` — el custodio solicita.
- `POST /api/v1/devoluciones/{id}/confirmar` `{ "ubicacion_id"? }` — el almacén acusa la recepción.
- `GET /api/v1/devoluciones` — bandeja de devoluciones pendientes (almacén).
- `POST /api/v1/transferencias` `{ "expediente_id", "destino_id" }` — el custodio solicita.
- `POST /api/v1/transferencias/{id}/aceptar` y `/rechazar` — solo el usuario destino.

Administración (rol ADMINISTRADOR):

- `GET /api/v1/admin/usuarios` y `POST /api/v1/admin/usuarios`
- `POST /api/v1/admin/usuarios/{id}/activar` y `/desactivar`
- `GET /api/v1/admin/roles`

Motivos de descarte: `NO_LOCALIZADO`, `NO_DISPONIBLE`, `EN_CUSTODIA_OTRO`, `REGISTRO_ERRONEO`, `OTRO`
(este último exige descripción).

## Sincronización offline

- `POST /api/v1/sync/dispositivos` `{ "nombre", "plataforma" }` — registra el dispositivo del usuario.
- `POST /api/v1/sync/push` `{ "dispositivo_id", "operaciones": [{ "clave", "tipo", "payload" }] }`
  — idempotente por (clave, dispositivo). Solo `CREAR_PEDIDO` se acepta sin conexión; cualquier
  confirmación de custodia, entrega, devolución o permiso se rechaza.
- `GET /api/v1/sync/pull?dispositivo_id=...&cursor=...&limite=...` — eventos ordenados y filtrados
  por permisos (global, por rol o por usuario).
- `GET /api/v1/sync/snapshot?dispositivo_id=...` — estado inicial autorizado y cursor.

El escritorio mantiene en SQLite las proyecciones autorizadas, el `outbox` con reintentos
exponenciales y la tabla `sync_conflicts` para los rechazos del servidor. El cursor local solo
avanza en la misma transacción que aplica los cambios.

## Escritorio

### Compilar el instalador (Windows)

Requisitos: Rust estable, Node.js 20+, Microsoft C++ Build Tools y WebView2 (incluido en Windows 10/11).

```bash
cd desktop
npm install
npm run check        # svelte-check
npm run tauri build
```

El instalador queda en:

```
desktop/src-tauri/target/release/bundle/nsis/Custodia CradSystem_0.1.0_x64-setup.exe
```

### Configurar el primer acceso

1. Abra la aplicación. La pantalla de acceso pide **Servidor**, correo y contraseña.
2. La dirección viene precargada con `https://129-121-117-177.sslip.io`; ajústela si su instalación usa otro nombre o el modo sin cifrado, sin barra final:
   - `https://129-121-117-177.sslip.io` si usa TLS,
   - `http://129.121.117.177` si usa el modo sin cifrado.
3. Use la cuenta administradora creada con `crear_admin`.
4. La dirección se guarda en la base local del equipo; para cambiarla, cierre sesión y edítela.
5. En **Administración** cree los usuarios y asigne roles.
6. Cada usuario entra con su contraseña temporal y la cambia con el botón **Contraseña** del menú lateral.

Si la URL empieza con `http://`, la aplicación muestra **«Sin cifrado»**: es intencional, indica que
el tráfico no está protegido.

La variable `EXPEDIENTES_API_URL` solo se usa como valor inicial en la primera ejecución; después
manda la dirección guardada en la pantalla de acceso.

### Operación

- Navegación por permisos: Inicio, Expedientes, Mis pedidos, Almacén, Entregas, Mis custodias,
  Devoluciones, Reportes y Administración.
- Alta de expedientes por cualquier usuario operativo: el creador queda como custodio inicial hasta que lo devuelva al almacén.
- Importación masiva desde CSV/TXT (`numero;descripcion`) para administración y almacén; los expedientes quedan en almacén y se reportan duplicados y errores.
- Reporte de expedientes nuevos por usuario, agrupado por día, mes y año.
- Expedientes con total, resumen por estado, filtros avanzados y paginación del servidor.
- Sesión de 18 horas y cierre automático tras 30 minutos sin actividad.
- Tablero con contadores, búsqueda de expedientes con historial, alta de pedidos múltiples,
  preparación y descarte, despacho, acuse, devoluciones, transferencias y administración.
- Todas las tablas tienen paginación (15 registros por página; 8 en los buscadores de los modales).
- Estado de sincronización visible (`En línea`, `Sin conexión`, `Error`), pendientes y conflictos.
- Sin conexión, la búsqueda usa la copia local y los pedidos nuevos se guardan en el outbox.

Los iconos se generan con `powershell -File scripts/generar-iconos.ps1` y se versionan en
`desktop/src-tauri/icons`.

## Estado por fases

Ver `docs/PLAN.md` y `docs/PROGRESO.md`.

- Fase 0: completada (diagnóstico y plan).
- Fase 1: modelo institucional con pruebas de invariantes.
- Fase 2: completada (Argon2id, JWT, refresh rotativo, RBAC, administración de usuarios).
- Fase 3: completada (entregas parciales, acuses, devoluciones con acuse del almacén,
  transferencias con acuse del receptor y cancelación de pedidos).
- Fase 4: completada (push idempotente, pull incremental por permisos, snapshot y motor local
  con outbox y conflictos).
- Fase 5: completada (interfaz Svelte 5 con Tailwind, todos los paneles y estados de sincronización).
- Fase 6: completada (Docker Compose con Caddy y TLS, respaldos/restauración, paquete NSIS y
  documentación de operación).

## Despliegue

```bash
cd infra
cp .env.example .env      # completar secretos y SITIO
chmod +x preparar-vps.sh
./preparar-vps.sh --admin-correo 'jefe@institucion.mx' --admin-password 'UnaClaveLargaYUnica#2026'
set -a; . ./.env; set +a
curl -fsS "https://$SITIO/health"
```

En un VPS con dominio público defina `CADDYFILE=./Caddyfile.publico` en `infra/.env` para TLS
automático con Let's Encrypt. Si no es posible configurar TLS, existe `Caddyfile.http` para redes
cerradas, con advertencia visible en la aplicación; el tráfico queda **sin cifrar**.

En un VPS administrado con **Portainer**, construya primero la imagen
(`docker compose -f docker-compose.yml build api`) y ejecute
`./preparar-vps.sh --portainer --sitio <dominio>`. El script genera `infra/portainer.env` con
`API_IMAGE`, `CADDYFILE` y `CERTS_DIR` en rutas absolutas; cree el stack *Repository* con
`infra/docker-compose.portainer.yml`, nombre `expedientes` y esas variables. Portainer no compila
la API en este flujo. Después compruebe el acceso con `bash ./verificar-tls.sh`. Ver
`docs/DESPLIEGUE.md` (secciones «Ruta rápida para VPS», «Sin TLS» y «Coexistencia con Portainer»),
`docs/OPERACION.md` y `docs/SEGURIDAD.md`.

## Documentación

`docs/PLAN.md`, `docs/PROGRESO.md`, `docs/ARQUITECTURA.md`, `docs/MODELO_DATOS.md`,
`docs/SEGURIDAD.md`, `docs/SINCRONIZACION.md`, `docs/DESPLIEGUE.md`, `docs/OPERACION.md`,
`docs/PRUEBAS.md`, `docs/MANUAL_USUARIO.md`.

## Cadena de custodia

La custodia oficial solo cambia con un acuse autenticado:

1. El despacho **no** transfiere custodia: queda `DESPACHADA` y el custodio anterior continúa siéndolo.
2. La recepción del solicitante crea el acuse y abre la nueva custodia (`RECIBIDA`).
3. La devolución no cierra la custodia hasta que el almacén confirma la recepción física.
4. La transferencia no cambia de custodio hasta que el usuario destino la acepta.

En todo momento existe **una sola custodia activa por expediente**, garantizada por un índice único
parcial en base de datos.
"# almacen" 
