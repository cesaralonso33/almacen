# Pruebas

## Servidor

```bash
cd server
export TEST_ADMIN_DATABASE_URL='postgres://postgres:clave@localhost:5432/postgres'
cargo test
```

Cada prueba crea una base PostgreSQL efímera, aplica todas las migraciones y la elimina.

| Suite | Pruebas | Cobertura |
|---|---|---|
| `tests/modelo_datos.rs` | 12 | RBAC, ubicaciones, reservas, descartes, auditoría inmutable, sync, idempotencia, custodia única, numeración |
| `tests/auth_rbac.rs` | 16 | login, refresh rotativo, logout, cambio de contraseña, 401/403, identidad del token, administración |
| `tests/custodia.rs` | 8 | devoluciones con acuse, transferencias, concurrencia, cancelación |
| `tests/sync.rs` | 10 | snapshot, push idempotente, pull filtrado, cursores, operación no permitida y CONSULTA de solo lectura |
| `tests/consultas.rs` | 7 | tablero, listados por rol, custodias, historial, directorio |
| `tests/bootstrap.rs` | 2 | creación del administrador inicial, duplicados y contraseña corta |
| `tests/expedientes.rs` | 4 | alta con custodia del creador, año actual, devolución, permisos y formato del número |
| `src/api/expedientes.rs` (unitarias) | 1 | normalización a 8 caracteres antes de la barra |
| `tests/admin.rs` | 2 | edición de nombre, correo y contraseña; revocación de sesiones |
| `tests/importacion.rs` | 2 | importación masiva, normalización, duplicados, permisos y límite |
| `tests/reportes.rs` | 2 | altas por usuario agrupadas por día, mes y año; permisos |
| `tests/busqueda.rs` | 4 | total, filtros avanzados, Unicode, comodines, paginación e índices trigram |
| `tests/sesion.rs` | 4 | duración de 24 h, inactividad de 18 h, actividad reciente y exclusión de sync |
| `tests/transferencias.rs` | 1 | bandeja de transferencias pendientes y aceptación |
| `tests/asignacion.rs` | 3 | pedido masivo a nombre de otro usuario, permiso y rechazos |
| `tests/seguridad.rs` | 3 | cabeceras, límite de cuerpo y credenciales de longitud abusiva |
| `tests/pa.rs` | 3 | CRUD P.A., folio anual, campos libres, filtros, permisos y PDF |

Total: **84 pruebas**.

## Escritorio (Rust)

```bash
cd desktop/src-tauri
cargo test
```

10 pruebas: 8 del motor de sincronización y 2 de cifrado SQLCipher (creación, reapertura, clave incorrecta y migración desde SQLite plana).

## Interfaz

```bash
cd desktop
npm test        # coordinación de cargas y single-flight
npm run check   # svelte-check
npm run build
```

`npm test` ejecuta 3 pruebas con el runner de Node.js.

## Infraestructura

```bash
bash -n infra/preparar-vps.sh
bash -n infra/verificar-tls.sh
python -c "import yaml; yaml.safe_load(open('infra/docker-compose.yml'))"
```

- `preparar-vps.sh --portainer` se probó en una copia temporal: genera `.env` y
  `portainer.env` con `API_IMAGE`, `CADDYFILE` y `CERTS_DIR` absolutos, normaliza `SITIO` y no
  requiere Docker. Rechaza `--admin-*` en ese modo.
- `verificar-tls.sh` se probó con Docker y curl simulados: detecta `Caddyfile.http` (código 3),
  TLS local fallido (código 1), TLS correcto (código 0) y una ruta de montaje inexistente
  (código 1).
- `infra/docker-compose.portainer.yml` se parsea con PyYAML; no usa `build` ni rutas relativas de
  Caddy.
- La validación real de contenedores, certificados, firewall y despliegue del stack de Portainer
  debe ejecutarse en el VPS; no está verificada en producción.

## Verificación manual de despliegue

1. `docker compose up -d --build`.
2. `set -a; . ./.env; set +a; curl -fsS "https://$SITIO/health"` → `{"ok":true}`.
3. Iniciar sesión con cada rol y comprobar la navegación visible.
4. Crear un pedido de 5 expedientes, entregar 2, descartar 1 y comprobar `PARCIAL`.
5. Entregar los 2 restantes y comprobar `COMPLETADO` (4 entregados + 1 descartado).
6. Solicitar una devolución y confirmar que la custodia sigue activa hasta el acuse del almacén.
7. Desconectar la red, crear un pedido, reconectar y sincronizar una sola vez.
8. Ejecutar `chmod +x backup/backup.sh && ./backup/backup.sh` y restaurar en una base de prueba.

## Limitaciones conocidas

- No hay prueba E2E automatizada de la ventana Tauri; la interfaz se valida con `svelte-check`,
  compilación y las pruebas de la API que consume.
- Docker no está disponible en el entorno de desarrollo usado, por lo que el despliegue se validó
  por sintaxis de Compose, scripts y configuración, no con `docker compose up`.
