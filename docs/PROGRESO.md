# Progreso real del proyecto

Última actualización: 2026-11-15

## Resumen

El repositorio contiene un **starter funcional pero de prototipo**: la API Rust compila, migra y
ejecuta el ciclo de pedido → preparación → descarte → entrega parcial → acuse → custodia →
devolución. No hay autenticación, ni pruebas automatizadas, ni sincronización, ni una UI real, y
el escritorio Tauri no compila por falta de iconos. No es desplegable.

## Entorno de diagnóstico

| Herramienta | Versión | Estado |
|---|---|---|
| cargo / rustc | 1.90.0 | disponible |
| node / npm | v22.18.0 / 11.19.1 | disponible |
| docker | — | **no disponible** |
| PostgreSQL | 14.5 (Laragon, localhost:5432) | disponible |
| git | disponible, pero el proyecto **no es repositorio** | — |

## Comandos ejecutados y resultados reales

| Comando | Directorio | Resultado |
|---|---|---|
| `cargo check` | `server/` | **OK**, 1 advertencia (`Postgres`, `Transaction` sin usar) |
| `cargo test` | `server/` | **OK**, `running 0 tests` |
| `npm install` | `desktop/` | OK, 43 paquetes, 3 vulnerabilidades bajas, aviso de scripts de `esbuild` |
| `npm run build` | `desktop/` | **OK**, `dist/` generado (38.36 kB JS) |
| `cargo check` | `desktop/src-tauri/` | **FALLA**: ``icons/icon.ico` not found; required for generating a Windows Resource file`` |
| `npm run check` | `desktop/` | no existe el script |

## Verificación funcional de la API

Base de prueba creada para la revisión: `expedientes_review` (PostgreSQL local, usuario `postgres`).
La API se ejecutó con `DATABASE_URL` apuntando a esa base y aplicó la migración `0001` al arrancar
(`_sqlx_migrations` = 1 fila). Se sembraron 4 usuarios y 6 expedientes ficticios.

Escenario ejecutado por HTTP (códigos reales):

1. `POST /pedidos` con 5 expedientes → **200**.
2. Detalle inicial → `PENDIENTE`, 0 entregados, 0 descartados.
3. Preparar 2 renglones → **200/200**; descartar 1 con motivo → **200**; motivo `"No"` → **400**.
4. `POST /entregas` con 2 renglones → **200**, `PENDIENTE_RECEPCION`. Pedido → `PARCIAL` (1 descartado).
5. Repetir despacho del mismo renglón → **409**.
6. Confirmar con usuario distinto al receptor → **403**; receptor correcto → **200**; repetir → **409**.
7. Preparar/despachar/confirmar los 2 restantes → pedido **COMPLETADO** con 4 entregados + 1 descartado.
8. Devolución por el custodio → **200** y el expediente vuelve a `ALMACEN` **sin acuse del almacén**.
9. Pedido nuevo sobre un expediente en custodia → **200** (debería rechazarse).
10. Pedido con expediente inexistente → **500** (debería ser 404/422).
11. Ruta inexistente → **404** con cuerpo vacío; método incorrecto → **405** con cuerpo vacío.
12. Concurrencia: dos despachos simultáneos del mismo renglón → **200 y 409**, una sola entrega.
13. Concurrencia: dos confirmaciones simultáneas del mismo acuse → **200 y 409**, **1 custodia activa**,
    3 movimientos, ninguna entrega duplicada.

Conclusión: los criterios 1, 2, 5 y 8 se sostienen; 3, 4 y 11 son parciales; 6, 7, 9, 10 y 12 no se
cumplen.

## Hallazgos por severidad

### Críticos

1. **Identidad suministrada por el cliente** (`server/src/main.rs:13-18`): `actor_id`,
   `solicitante_id` y `receptor_id` vienen del cuerpo. Sin login no hay cadena de custodia válida.
2. **Devolución sin acuse del almacén** (`main.rs:138-152`): el custodio cierra la custodia y deja el
   expediente en `ALMACEN` en una sola llamada. Viola el criterio 6 y el modelo de doble confirmación.
3. **Sin RBAC**: cualquier usuario puede preparar, descartar, despachar y confirmar por cualquiera.
4. **Escritorio no compila** (falta `src-tauri/icons/icon.ico`), por lo que no hay aplicación instalable.
5. **Sin control de versiones**: no existe `.git`; cualquier cambio es irreversible.

### Altos

6. Errores inconsistentes: FK inexistente → **500**; framework → 400/404/405 con cuerpo vacío.
   No hay tipo de error uniforme `{code, message, correlation_id}`.
7. `POST /pedidos` no valida existencia ni disponibilidad de los expedientes; acepta expedientes ya
   entregados o en custodia (verificado).
8. No hay reservas ni vencimiento: `PREPARADO` es el único bloqueo y no caduca (`main.rs:63-80`).
9. No hay idempotencia efectiva: la tabla `processed_commands` existe pero ningún endpoint la usa;
   un reintento de red puede duplicar movimientos.
10. `entregas.estado` usa `PENDIENTE_RECEPCION`/`CONFIRMADA` en lugar de `DESPACHADA`/`RECIBIDA`
    exigidos; `pedido_items` no admite `RESERVADO` ni `CANCELADO`; no existe `CANCELADO` de pedido.
11. `expedientes.numero` es `UNIQUE` global; el requisito pide unicidad parametrizable por unidad/año.
12. Inversión de orden de bloqueo: `prepare` bloquea renglón → expediente (`main.rs:65-70`) y
    `dispatch` bloquea expediente → renglón (`main.rs:91-98`). Riesgo de deadlock bajo carga;
    no se reprodujo en 12 intentos con dos procesos, queda como riesgo abierto.
13. Sin tabla de auditoría append-only: `movimientos` es mutable y no guarda resultado, ubicación,
    dispositivo, IP ni ID de correlación.

### Medios

14. Escritorio: la outbox serializa el payload como `"numero|descripcion"` (`desktop/src-tauri/src/lib.rs:23`),
    no hay motor de sincronización, ni migraciones locales versionadas, ni cifrado de SQLite.
15. Escritorio: `crear_expediente_local` usa el estado `PENDIENTE_SYNC`, que el servidor no conoce;
    `numero UNIQUE` local chocará con números ya sincronizados.
16. UI (`desktop/src/App.svelte`): 20 líneas, sin navegación, autenticación, pedidos, entregas,
    estados de conexión, accesibilidad ni Tailwind.
17. `package.json` no incluye Tailwind, `svelte-check`, pruebas ni script `check`; `bundle.active=false`
    en `tauri.conf.json` (no se genera instalador).
18. Infra: solo PostgreSQL; faltan API, Caddy, healthchecks, `.env.example`, backups y restauración.
19. API: sin configuración (`DATABASE_URL` por entorno), puerto `0.0.0.0:3000` fijo, sin CORS, sin TLS,
    sin límite de tasa, sin apagado ordenado, `/health` no comprueba la base.
20. Sin `docs/`, `scripts/`, `tests/`, `.gitignore` ni `.env.example`.

## Avance 2026-11-15 — segundo bloque

### Control de versiones

- `git init -b main` y `.gitignore` (Rust, Node, entornos, bases locales, esquemas generados de Tauri).
- Commit inicial `9cbe5fa` con 37 archivos versionados.

### Escritorio

- `scripts/generar-iconos.ps1` genera `icon.ico` (PNG 256 incrustado) y los PNG que espera Tauri.
- `cargo check` en `desktop/src-tauri` → **OK** (antes fallaba por `icons/icon.ico` ausente).

### Modelo de datos (migración `0002_modelo_institucional.sql`)

Añadido, sin eliminar datos:

- `usuarios` (renombrada desde `users`) con correo, hash de contraseña, estado y marcas de tiempo.
- RBAC: `roles`, `permisos`, `rol_permisos`, `usuario_roles` con índice único de asignación activa.
- `dispositivos`, `sesiones` (refresh único, revocación, reemplazo) y `sync_devices`.
- `ubicaciones` jerárquicas EDIFICIO > AREA > ESTANTE > CAJA con trigger de validación y
  unicidad de código por padre (incluido el nivel raíz).
- `expedientes`: `unidad`, `anio`, `ubicacion_id`; la unicidad de `numero` ahora es por
  unidad/año mediante índice único.
- `reservas` con `expira_en` y una sola reserva activa por expediente.
- `motivos_descarte` (5 claves) y `descartes` con detalle obligatorio para `OTRO`.
- `acuses` e `incidencias`.
- `auditoria` append-only con reglas `DO INSTEAD NOTHING` para UPDATE y DELETE.
- `sync_events` con secuencia global y tombstones; `operaciones_idempotentes` por
  (clave, dispositivo).
- Semillas de roles, 17 permisos, motivos de descarte y una ubicación base.

### Pruebas

`cargo test` (12 pruebas de integración, base efímera por prueba): **12 passed, 0 failed**.
Verificado también que la migración `0002` se aplica sobre la base ya poblada de la revisión
(2 migraciones) y que el flujo pedido → preparar → despachar → confirmar sigue devolviendo
`COMPLETADO` con una custodia activa.

### Pendiente inmediato

- Alinear `entregas.estado` y `pedido_items.estado` con el modelo exigido (DESPACHADA/RECIBIDA,
  RESERVADO/CANCELADO) al reescribir los endpoints en Fase 2–3.
- Semillas de usuarios de demostración con Argon2id (requiere el código de autenticación).
- Conectar `reservas` a `prepare`/`dispatch` (hoy solo existe la tabla).

### Autenticación y RBAC (Fase 2)

Reestructuración del servidor en biblioteca + binario:

```
server/src/
  lib.rs, main.rs, config.rs, error.rs, limiter.rs, audit.rs
  auth/{mod,password,tokens,extractor}.rs
  api/{mod,entrada,auth,admin,pedidos,entregas,expedientes}.rs
  bin/seed_demo.rs
```

- Argon2id, JWT HS256 de 15 minutos ligado a `sesiones.id`, refresh opaco rotativo con hash SHA-256
  y detección de reuso (el refresh anterior devuelve 401 tras rotar).
- Extraer identidad del token: se eliminaron `actor_id`, `solicitante_id` y `receptor_id` de los
  cuerpos; `deny_unknown_fields` hace que enviarlos devuelva 400.
- Permisos verificados en cada endpoint (`pedidos:crear`, `pedidos:preparar`, `entregas:despachar`,
  `entregas:confirmar`, `devoluciones:solicitar`, `admin:usuarios`, ...).
- Limitador de login: 5 fallos → 429.
- Cambio de contraseña revoca todas las sesiones; desactivar un usuario revoca y bloquea el acceso.
- Errores uniformes también en 404, 405 y JSON inválido; se añadió `fallback` y
  `method_not_allowed_fallback`.
- `seed_demo` crea 4 cuentas y 6 expedientes ficticios con Argon2id; es idempotente
  (verificado: 4 usuarios, 6 expedientes y 4 asignaciones tras ejecutarlo dos veces).

### Pruebas de la Fase 2

`cargo test`: **28 pruebas, 0 fallos** (16 de autenticación/RBAC + 12 de modelo de datos).

Verificación manual contra PostgreSQL real (`expedientes_fase2`, API en el puerto 3000):

| Escenario | Resultado |
|---|---|
| Login de solicitante, almacén y consulta | 200 con permisos distintos por rol |
| `GET /auth/me` sin token | 401 `NO_AUTENTICADO` |
| `POST /pedidos` con `solicitante_id` en el cuerpo | 400 |
| Pedido creado por sesión | 200, solicitante = usuario del token |
| Preparar con perfil CONSULTA | 403 `SIN_PERMISO` |
| Despacho y confirmación por el receptor | 200, pedido `COMPLETADO` |
| Confirmación por el almacén | 403 |
| Refresh reutilizado | 401 |
| Logout y reutilización del access | 401 |
| Ruta inexistente / método incorrecto | 404 / 405 con JSON uniforme |
| Auditoría | 7 eventos con actor y resultado |
| Sesiones | 4 creadas, 2 revocadas |

### Pendientes conocidos de la Fase 2

- La API no emite tokens para dispositivos: `sesiones.dispositivo_id` queda nulo hasta la Fase 4.
- `debe_cambiar_password` se informa en el login, pero no bloquea las demás rutas.
- No hay CORS ni TLS: se resuelven en la Fase 6 dentro de la red institucional.

### Devoluciones, transferencias y cancelación (Fase 3)

Migración `0003_devoluciones_y_transferencias.sql`:

- `entregas.estado` pasa a `PREPARADA | DESPACHADA | RECIBIDA | CANCELADA | INCIDENCIA`; los datos
  existentes se convierten (`PENDIENTE_RECEPCION` → `DESPACHADA`, `CONFIRMADA` → `RECIBIDA`).
- `pedido_items` admite `RESERVADO` y `CANCELADO`, con `cancelado_motivo`.
- `pedidos.cancelado_en`, `cancelado_por`, `cancelado_motivo` con restricción de coherencia.
- `devoluciones` (SOLICITADA/CONFIRMADA/CANCELADA) con índice único de una solicitud activa.
- `transferencias` (SOLICITADA/ACEPTADA/RECHAZADA/CANCELADA) con índice único de una activa.
- Permiso `transferencias:solicitar` para ADMINISTRADOR, ALMACEN y USUARIO.

Endpoints nuevos:

- `POST /devoluciones` y `POST /devoluciones/{id}/confirmar`, `GET /devoluciones`.
- `POST /transferencias` y `POST /transferencias/{id}/aceptar|rechazar`.
- `POST /pedidos/{id}/cancelar`.
- Se retiró `POST /expedientes/{id}/devolver`, que permitía cerrar la custodia sin acuse.

Pruebas nuevas (`tests/custodia.rs`, 8):

1. La devolución no cierra la custodia hasta el acuse del almacén; el custodio recibe 403 y un
   segundo acuse devuelve 409.
2. Solo el custodio puede solicitar la devolución (409 en caso contrario).
3. La transferencia no cambia de custodio hasta la aceptación; el origen no puede aceptar (403).
4. El rechazo no altera la custodia.
5. Dos aceptaciones simultáneas → 200 y 409, una sola custodia y una sola transferencia aceptada.
6. Devolución y transferencia son mutuamente excluyentes (409).
7. Cancelar un pedido libera reservas, marca los renglones como CANCELADO y conserva el pedido.
8. No se cancela un pedido con renglones entregados o en tránsito (409).

`cargo test`: **36 pruebas, 0 fallos** (16 auth/RBAC + 8 custodia + 12 modelo).

Verificación manual sobre `expedientes_fase2` (base con datos de la Fase 2):

| Escenario | Resultado |
|---|---|
| Migración 0003 sobre datos existentes | entrega `CONFIRMADA` quedó como `RECIBIDA` |
| Devolución solicitada | 200, custodia activa sigue en 1 |
| Acuse del almacén | 200, custodia activa 0, expediente `ALMACEN` |
| Segundo acuse | 409 |
| Transferencia solicitada | 200, custodio sigue siendo el origen |
| Aceptación por el destino | 200, custodio `admin@demo.mx`, una sola custodia |
| Cancelación de pedido | 200, estado `CANCELADO`, renglón `CANCELADO` |
| Bandeja `/devoluciones` | 200 |

### Sincronización offline (Fase 4)

Servidor:

- Migración `0004`: `pedidos.version`, índice de eventos por usuario y reglas que hacen
  `sync_events` inmutable.
- `POST /sync/dispositivos`, `POST /sync/push`, `GET /sync/pull`, `GET /sync/snapshot`.
- Los eventos se emiten dentro de la transacción de negocio en pedidos, ítems, entregas,
  devoluciones y transferencias.
- Visibilidad: `GLOBAL` (expedientes/custodia), `ROL` (`ALMACEN`) y `USUARIO` (solicitante).
- Push idempotente por (clave, dispositivo): replay del resultado, rechazo de duplicados
  concurrentes y `operaciones_idempotentes` como registro.
- Solo `CREAR_PEDIDO` se acepta offline; cualquier otra operación devuelve `RECHAZADA`.

Escritorio (`desktop/src-tauri/src/{db,sync,api}.rs`):

- Esquema SQLite v1 con proyecciones (`expedientes`, `pedidos`, `pedido_items`, `entregas`),
  `outbox`, `sync_metadata`, `sync_conflicts` y `sync_status`.
- El lote de eventos y el cursor se escriben en una sola transacción; si un evento es inválido,
  se revierte todo y el cursor no avanza (probado).
- Retroceso exponencial con jitter para reintentos; los rechazos crean conflicto y no se reenvían.
- Snapshot cuando el cursor ya no es válido.
- Comandos Tauri: `login`, `logout`, `estado_sync`, `sincronizar`, `crear_pedido_local`,
  `listar_expedientes`, `listar_conflictos`.

Pruebas:

- Servidor: **45 pruebas** (16 auth/RBAC + 8 custodia + 12 modelo + 9 sync).
- Escritorio: **8 pruebas** del motor de sincronización.
- Casos cubiertos: snapshot, paginación y orden, cursor inválido, filtrado por usuario,
  idempotencia, duplicado concurrente, operación no permitida offline, dispositivo ajeno,
  corte de red con outbox persistente, lote inválido sin avance de cursor.

Verificación manual contra `expedientes_fase2`:

| Escenario | Resultado |
|---|---|
| Flujo completo tras añadir eventos | preparar/despachar/confirmar 200 |
| Registro de dispositivo | 200 con `dispositivo_id` |
| Push `CREAR_PEDIDO` | `ACEPTADA`, pedido creado |
| Replay con la misma clave | `ACEPTADA`, mismo `pedido_id`, sin duplicado |
| Pull con cursor 0 | eventos ordenados y filtrados por usuario |
| Snapshot | 6 expedientes, 5 pedidos, cursor |

Limitaciones conocidas:

- No hay SSE/WebSocket: el pull periódico recupera los cambios; es suficiente para la fase.
- Los tokens viven en memoria del proceso; al reiniciar la aplicación se requiere iniciar sesión.
- La interfaz Svelte todavía no consume los comandos (Fase 5).

### Interfaz Svelte 5 (Fase 5)

- Tailwind CSS 3 + PostCSS; `svelte.config.js` con `vitePreprocess`.
- Componentes: `Login`, `Shell` (navegación por permisos, barra de sincronización, conflictos),
  `Modal`, `Insignia` y nueve paneles.
- Endpoints de consulta añadidos en el servidor: tablero, pedidos, custodias, expedientes,
  historial, entregas pendientes y directorio de usuarios.
- El detalle de pedido ahora incluye número y descripción del expediente de cada renglón.
- Comando Tauri `peticion`: proxy autenticado con lista blanca de rutas `/api/v1/`; la interfaz
  nunca recibe el token.
- Sin conexión: la tabla de expedientes cae a la copia local con aviso; el alta de pedido se
  guarda en el outbox y se marca como pendiente.

Verificación:

| Comprobación | Resultado |
|---|---|
| `npm run check` (svelte-check) | **0 errores, 0 advertencias** |
| `npm run build` | OK, 85 kB JS y 19.5 kB CSS |
| `cargo build` escritorio | OK |
| `cargo test` servidor | **52 pruebas, 0 fallos** |
| `cargo test` escritorio | **8 pruebas, 0 fallos** |

Limitación honesta: no se ejecutó la ventana Tauri de forma automatizada, por lo que los flujos
están verificados por compilación y por las pruebas de la API que consumen, no por una prueba E2E
de interfaz. Queda para la Fase 6 junto con la instalación.

### Despliegue institucional (Fase 6)

- `server/Dockerfile` multi-etapa; `expedientes-api`, `seed_demo` y `crear_admin` en imagen Debian slim con usuario 10001.
- `infra/docker-compose.yml`: PostgreSQL 17, API y Caddy con healthchecks, reinicio automático y
  red interna; solo Caddy publica 80/443.
- `infra/Caddyfile` (certificados institucionales) y `Caddyfile.interno` (CA interna de Caddy).
- `infra/backup/backup.sh` y `infra/restore/restore.sh` con retención, confirmación y respaldo previo.
- `.env.example`, `.gitattributes` (LF en scripts) y `.dockerignore`.
- `crear_admin` para aprovisionar la cuenta administradora inicial sin datos de demostración.
- Paquete NSIS activado, iconos y CSP; dirección del servidor configurable desde el acceso y
  persistida en SQLite (`servidor_actual` / `configurar_servidor`).
- Documentación: `DESPLIEGUE.md`, `OPERACION.md`, `SEGURIDAD.md`, `ARQUITECTURA.md`,
  `MODELO_DATOS.md`, `SINCRONIZACION.md`, `PRUEBAS.md`.

Verificación:

| Comprobación | Resultado |
|---|---|
| Compose YAML | válido; servicios `postgres`, `api`, `caddy`; sin puertos en base ni API |
| `bash -n` de respaldo y restauración | OK, sin CRLF |
| `tauri.conf.json` | aceptado por `cargo check` |
| `npm run check` / `npm run build` | 0 errores / build OK |
| `cargo test` servidor | 65 pruebas en verde |
| `cargo test` escritorio | 8 pruebas en verde |

Limitación: **Docker no está instalado en este equipo**, por lo que `docker compose up`, la
construcción de la imagen y la emisión TLS no se ejecutaron aquí. El paquete NSIS tampoco se
generó (requiere herramientas de empaquetado y red). Queda como verificación pendiente en el
servidor Debian de destino.

## Estado frente a las fases

- Fase 0: **completada** (este documento y `docs/PLAN.md`).
- Fase 1: **parcial**. Modelo institucional y pruebas de invariantes listos; falta revisar el uso de
  `movimientos` frente a `auditoria`.
- Fase 2: **completada**. Autenticación, sesiones rotativas, RBAC, administración y auditoría.
- Fase 3: **completada**. Devoluciones con acuse, transferencias con aceptación, cancelación y
  estados operativos.
- Fase 4: **completada**. Push idempotente, pull incremental filtrado, snapshot y motor local con
  outbox y conflictos.
- Fase 5: **completada**. Interfaz Svelte 5 con Tailwind, paneles por rol y estados de
  sincronización; pendiente la prueba E2E de la ventana.
- Fase 6: **completada** en artefactos y documentación; verificación con Docker pendiente en el
  servidor de destino.

## Siguiente acción recomendada

1. Inicializar git y añadir `.gitignore`.
2. Corregir el build del escritorio (iconos) para poder verificar la cadena completa.
3. Migración `0002` con RBAC, sesiones, ubicaciones, reservas, catálogos y auditoría.
4. Sustituir `actor_id` por identidad de sesión y añadir el tipo de error uniforme.
5. Implementar devolución con acuse de almacén y transferencias con acuse de receptor.
6. Añadir suite de integración Rust contra PostgreSQL (criterios 1–6 y concurrencia).

## Limitaciones del diagnóstico

- No hay Docker en el equipo, por lo que el `docker-compose.yml` no se pudo validar.
- La base de prueba quedó en `expedientes_review`; los datos son ficticios.
- El escritorio no se pudo ejecutar por el fallo de compilación indicado.
