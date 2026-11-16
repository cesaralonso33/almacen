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
- `POST /api/v1/entregas/{id}/confirmar` — solo el receptor designado.
- `POST /api/v1/expedientes/{id}/devolver` — solo el custodio actual.

Administración (rol ADMINISTRADOR):

- `GET /api/v1/admin/usuarios` y `POST /api/v1/admin/usuarios`
- `POST /api/v1/admin/usuarios/{id}/activar` y `/desactivar`
- `GET /api/v1/admin/roles`

Motivos de descarte: `NO_LOCALIZADO`, `NO_DISPONIBLE`, `EN_CUSTODIA_OTRO`, `REGISTRO_ERRONEO`, `OTRO`
(este último exige descripción).

## Escritorio

```bash
cd desktop
npm install
npm run build        # frontend
npm run tauri dev    # aplicación de escritorio
```

Los iconos se generan con `powershell -File scripts/generar-iconos.ps1` y se versionan en
`desktop/src-tauri/icons`.

## Estado por fases

Ver `docs/PLAN.md` y `docs/PROGRESO.md`.

- Fase 0: completada (diagnóstico y plan).
- Fase 1: modelo institucional con pruebas de invariantes; pendiente alinear estados y conectar
  reservas al resto del ciclo.
- Fase 2: completada (Argon2id, JWT, refresh rotativo, RBAC, administración de usuarios).
- Fases 3–6: pendientes. La devolución todavía no exige acuse del almacén y no existen transferencias.
