# Expedientes jurídicos — almacén y custodia

Sistema para solicitud, entrega, devolución y cadena de custodia de expedientes jurídicos físicos.
Estado actual: **prototipo funcional en construcción**. La API y el escritorio compilan y el flujo
principal opera contra PostgreSQL, pero **todavía no hay autenticación ni RBAC aplicado en las rutas**,
por lo que no debe exponerse fuera de un entorno de desarrollo.

## Requisitos

- Rust estable (probado con 1.90)
- Node.js 20+ y npm
- PostgreSQL 14+ (probado con 14.5 local)
- Dependencias de Tauri 2 para el sistema operativo

## Base de datos

Con Docker Compose (recomendado para desarrollo):

```bash
cd infra
printf 'POSTGRES_PASSWORD=clave_de_desarrollo\n' > .env
docker compose up -d
export DATABASE_URL='postgres://expedientes:clave_de_desarrollo@localhost:5432/expedientes'
```

O con un PostgreSQL local:

```bash
createdb expedientes
export DATABASE_URL='postgres://usuario:clave@localhost:5432/expedientes'
```

## API

```bash
cd server
export DATABASE_URL='postgres://...'
cargo run
```

Las migraciones se aplican al iniciar. La API escucha en `http://localhost:3000` y
`GET /health` comprueba disponibilidad.

### Pruebas

```bash
cd server
export TEST_ADMIN_DATABASE_URL='postgres://postgres:clave@localhost:5432/postgres'
cargo test
```

Cada prueba crea una base efímera, aplica todas las migraciones y la elimina. Cubre RBAC,
sesiones, jerarquía de ubicaciones, reservas, descartes catalogados, auditoría append-only,
sincronización, idempotencia, custodia única y unicidad de números de expediente.

## Escritorio

```bash
cd desktop
npm install
npm run build        # frontend
npm run tauri dev    # aplicación de escritorio
```

Los iconos se generan con `powershell -File scripts/generar-iconos.ps1` y se versionan en
`desktop/src-tauri/icons`.

## Endpoints actuales

- `POST /api/v1/pedidos` `{ "solicitante_id":"UUID", "expedientes":["UUID"] }`
- `GET /api/v1/pedidos/{id}`
- `POST /api/v1/items/{id}/preparar` `{ "actor_id":"UUID" }`
- `POST /api/v1/items/{id}/descartar` `{ "actor_id":"UUID", "motivo":"..." }`
- `POST /api/v1/entregas` `{ "actor_id":"UUID", "item_ids":["UUID"] }`
- `POST /api/v1/entregas/{id}/confirmar` `{ "receptor_id":"UUID" }`
- `POST /api/v1/expedientes/{id}/devolver` `{ "actor_id":"UUID" }`

**Advertencia de seguridad:** las rutas reciben la identidad en el cuerpo (`actor_id`,
`solicitante_id`, `receptor_id`). Cualquiera con acceso a la API puede suplantar usuarios. La
Fase 2 del plan sustituye estos campos por identidad de sesión autenticada.

## Estado por fases

Ver `docs/PLAN.md` y `docs/PROGRESO.md`.

- Fase 0: completada (diagnóstico y plan).
- Fase 1: modelo institucional creado (migración `0002`), pendiente cerrar la alineación de
  estados y las semillas de usuarios de demostración.
- Fases 2–6: pendientes.
