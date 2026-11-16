# Expedientes jurídicos — almacén y custodia

Sistema para solicitud, entrega, devolución y cadena de custodia de expedientes jurídicos físicos.
Estado actual: **prototipo funcional con autenticación**. La API autentica con Argon2id + JWT,
aplica permisos por rol, registra auditoría y ejecuta el ciclo de pedidos y custodia. La interfaz
Svelte todavía no consume la API (Fase 5).

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

```bash
cd desktop
npm install
npm run check        # svelte-check
npm run build        # frontend
npm run tauri dev    # aplicación de escritorio
```

Variable `EXPEDIENTES_API_URL` (por omisión `http://127.0.0.1:3000`).

La interfaz incluye:

- Acceso con credenciales institucionales y sesión en memoria del proceso principal.
- Navegación por permisos: Inicio, Expedientes, Mis pedidos, Almacén, Entregas, Mis custodias,
  Devoluciones, Reportes y Administración.
- Tablero con contadores, tabla de expedientes con búsqueda e historial, alta de pedidos múltiples,
  bandeja de almacén con preparación y descarte, despacho, acuse de recepción, devoluciones,
  transferencias, administración de usuarios y roles.
- Estado de sincronización visible (`En línea`, `Sin conexión`, `Error`), pendientes y conflictos.
- Sin conexión, la búsqueda de expedientes usa la copia local y los pedidos nuevos se guardan en el
  outbox marcados como pendientes de sincronizar.

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
docker compose up -d --build
docker compose exec api env \
  ADMIN_CORREO='jefe@institucion.mx' \
  ADMIN_PASSWORD='UnaClaveLargaYUnica#2026' \
  /usr/local/bin/crear_admin
curl https://$SITIO/health
```

En un VPS con dominio público use `cp Caddyfile.publico Caddyfile` para TLS automático con
Let's Encrypt. Ver `docs/DESPLIEGUE.md` (sección «Ruta rápida para VPS»), `docs/OPERACION.md` y
`docs/SEGURIDAD.md`.

## Documentación

`docs/PLAN.md`, `docs/PROGRESO.md`, `docs/ARQUITECTURA.md`, `docs/MODELO_DATOS.md`,
`docs/SEGURIDAD.md`, `docs/SINCRONIZACION.md`, `docs/DESPLIEGUE.md`, `docs/OPERACION.md`,
`docs/PRUEBAS.md`.

## Cadena de custodia

La custodia oficial solo cambia con un acuse autenticado:

1. El despacho **no** transfiere custodia: queda `DESPACHADA` y el custodio anterior continúa siéndolo.
2. La recepción del solicitante crea el acuse y abre la nueva custodia (`RECIBIDA`).
3. La devolución no cierra la custodia hasta que el almacén confirma la recepción física.
4. La transferencia no cambia de custodio hasta que el usuario destino la acepta.

En todo momento existe **una sola custodia activa por expediente**, garantizada por un índice único
parcial en base de datos.
"# almacen" 
