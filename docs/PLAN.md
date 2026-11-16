# Plan de trabajo — Almacén y custodia de expedientes jurídicos

Estado de partida: **starter mínimo** (1 migración, 1 archivo `main.rs` con 7 rutas, UI de 20 líneas).
Este plan sustituye al alcance del README del starter y se ordena por dependencias reales.

## Fase 0 — Diagnóstico (completada)

Objetivo: medir qué existe, qué compila y qué falla antes de tocar código.

Entregables: este plan, `docs/PROGRESO.md`, inventario de hallazgos.

Criterio de salida: compilación de API y escritorio ejecutada al menos una vez y resultados registrados sin invenciones.

Riesgos detectados en el starter (prioridad alta):

1. **Sin autenticación**: todas las mutaciones reciben `actor_id`/`receptor_id`/`solicitante_id` del cliente. Cualquiera con acceso a la API puede actuar como cualquier usuario.
2. **Sin RBAC ni sesiones**: no existen tablas de roles, permisos, sesiones, dispositivos.
3. **Errores inconsistentes**: violaciones de FK devuelven 500; axum devuelve 400/404/405 con cuerpo vacío.
4. **Escritorio no compila**: `tauri-build` falla por `icons/icon.ico` ausente.
5. **Sin control de versiones**: el directorio no es un repositorio git.
6. **Sin pruebas**: `cargo test` ejecuta 0 pruebas; no hay `tests/` ni runner en el escritorio.

## Fase 1 — Modelo de datos PostgreSQL

Depende de: Fase 0. Estado: **parcial** (migración `0002` aplicada y 12 pruebas en verde).

Trabajo pendiente:

- Alinear `entregas.estado` (`DESPACHADA`, `RECIBIDA`, `INCIDENCIA`) y `pedido_items.estado`
  (`RESERVADO`, `CANCELADO`) con el modelo exigido.
- Semillas de usuarios de demostración con Argon2id (depende del código de Fase 2).
- Conectar la tabla `reservas` a los endpoints de preparación y despacho.
- Rehacer `movimientos` como bitácora operativa completa (resultado, ubicación, dispositivo,
  correlación) o sustituirla por `auditoria` en los endpoints.

- Migración `0002` con: `roles`, `permisos`, `usuario_roles`, `sesiones`, `dispositivos`, `ubicaciones` jerárquicas, catálogos de estado, `reservas`, `acuses`, `incidencias`, `descartes`, `auditoria`, `sync_events`, `sync_devices`, `operaciones_idempotentes`.
- Rehacer `pedidos`/`pedido_items`/`entregas` para soportar `RESERVADO`, `CANCELADO`, `DESPACHADA`, `RECIBIDA`, `INCIDENCIA` y descartes con catálogo de motivos.
- Unicidad de `expedientes.numero` parametrizada por unidad/año (no UNIQUE global).
- Índice único parcial de custodia activa ya existe: conservarlo y probarlo.
- Semillas de desarrollo con contraseñas de demostración documentadas.

Criterio de salida: `sqlx migrate run` desde base vacía + pruebas de restricciones (custodia única, doble reserva, FK, CHECK) en verde.

## Fase 2 — Autenticación y RBAC

Depende de: Fase 1. Estado: **completada**.

Implementado:

- Argon2id para contraseñas; hash nunca se expone.
- JWT HS256 de acceso (15 min) con `sub` y `sid`; validación contra sesión viva en base.
- Refresh opaco de 32 bytes, almacenado como SHA-256, rotación atómica con detección de reuso.
- Logout, `/me`, cambio de contraseña con revocación de todas las sesiones.
- Limitador de login (5 fallos por correo en 15 min) → 429.
- Extractor `AuthUser` con permisos cargados de la base en cada petición.
- `actor_id`, `solicitante_id` y `receptor_id` eliminados de los cuerpos; campos desconocidos → 400.
- Permisos aplicados en cada ruta; receptor y custodio se validan contra la sesión.
- Administración de usuarios y roles (`/admin/...`), activación/desactivación con revocación.
- Errores uniformes (`code`, `message`, `correlacion_id`) también en 404/405 y JSON inválido.
- Auditoría transaccional de login, refresh, logout, cambio de contraseña y operaciones de negocio.
- Semilla `seed_demo` con Argon2id y contraseñas de demostración documentadas.

Criterio de salida: criterio de aceptación 9 en verde y 401/403 probados. **Cumplido** con 16 pruebas
de integración de autenticación y RBAC.

## Fase 3 — Flujo de pedidos, entregas y custodia

Depende de: Fase 2. Estado: **completada**.

Implementado:

- Migración `0003`: estados de entrega (`DESPACHADA`, `RECIBIDA`, `CANCELADA`, `INCIDENCIA`),
  `RESERVADO`/`CANCELADO` en renglones, cancelación de pedidos y tablas `devoluciones` y
  `transferencias` con índices únicos parciales.
- Devolución en dos pasos: el custodio solicita y **la custodia no cambia** hasta que el almacén
  acusa la recepción, momento en que se cierra y se actualiza la ubicación.
- Transferencia directa en dos pasos: el custodio solicita y el custodio cambia solo cuando el
  destino acepta; rechazo no altera nada.
- Devolución y transferencia mutuamente excluyentes por expediente.
- Cancelación de pedido con motivo, liberación de reservas y conservación del historial.
- Estados de pedido derivados: `PENDIENTE`, `EN_PROCESO`, `PARCIAL`, `COMPLETADO`, `CANCELADO`.
- Todas las transiciones registran movimiento y auditoría en la misma transacción.

Criterio de salida: criterios de aceptación 1–6 y pruebas de concurrencia. **Cumplido** con
8 pruebas adicionales (36 en total).

## Fase 4 — Sincronización offline

Depende de: Fases 1–3. Estado: **completada** (sin notificaciones SSE; el pull cubre la recuperación).

Implementado:

- Migración `0004`: versión de pedidos e índices de eventos; `sync_events` inmutable.
- Registro de dispositivos por usuario y validación de propiedad en cada push/pull.
- `sync_events` emitidos en la misma transacción que cada operación de negocio, con visibilidad
  `GLOBAL`, `ROL` o `USUARIO`.
- Push idempotente por (clave, dispositivo) con replay del resultado y rechazo de operaciones
  concurrentes duplicadas.
- Pull incremental paginado, ordenado, filtrado por permisos y con `reiniciar` cuando el cursor
  ya no es válido; snapshot inicial autorizado.
- Solo `CREAR_PEDIDO` se acepta offline; toda confirmación de custodia, entrega, devolución o
  permiso se rechaza con `RECHAZADA`.
- Escritorio: esquema SQLite versionado, proyecciones, `outbox` con retroceso exponencial,
  `sync_conflicts` y `sync_status`; el cursor avanza en la misma transacción que aplica el lote.
- Comandos Tauri: `login`, `logout`, `estado_sync`, `sincronizar`, `crear_pedido_local`,
  `listar_expedientes`, `listar_conflictos`.

Criterio de salida: criterios 7, 8 y 10. **Cumplido** con 9 pruebas de servidor y 8 del motor local.

## Fase 5 — Interfaz Svelte 5

Depende de: Fases 2–4. Estado: **completada** (sin prueba E2E automatizada de la ventana Tauri).

Implementado:

- Tailwind CSS y sistema de componentes (`btn`, `campo`, `tarjeta`, `tabla`, `insignia`).
- Login, cierre de sesión y guardia por permisos en la navegación.
- Paneles: Inicio, Expedientes (búsqueda + historial), Mis pedidos (alta múltiple, detalle y
  cancelación), Almacén (preparar/descartar/despachar), Entregas (acuse), Mis custodias
  (devolución y transferencia), Devoluciones (acuse del almacén), Reportes y Administración.
- Barra de sincronización con estado, pendientes y conflictos; sincronización manual y automática
  cada 60 segundos.
- Sin conexión: búsqueda sobre la copia local y alta de pedido en el outbox con aviso explícito.
- Endpoints de consulta nuevos: `/dashboard`, `/pedidos` (GET), `/custodias`, `/expedientes`,
  `/expedientes/{id}/historial`, `/entregas/pendientes`, `/usuarios/directorio`.

Criterio de salida: flujos 1–6 desde la interfaz. **Cumplido a nivel de código y compilación**;
la verificación funcional de la ventana queda pendiente para la Fase 6 (prueba de instalación).

## Fase 6 — Despliegue institucional

Depende de: Fases 1–5. Estado: **completada** en configuración y artefactos; no ejecutada con
Docker en este entorno (no disponible).

Implementado:

- `server/Dockerfile` multi-etapa (Rust 1.90 → Debian slim, usuario sin privilegios).
- Compose con PostgreSQL 17, API y Caddy; healthchecks, reinicio automático y red interna sin
  puertos publicados para base y API.
- Caddyfile con TLS institucional, HSTS y cabeceras de seguridad; variante con CA interna.
- Respaldos con `pg_dump` y retención configurable; restauración con respaldo previo y confirmación.
- `.env.example` sin secretos, `.gitattributes` con LF para scripts y `.dockerignore`.
- Paquete NSIS configurado (`bundle.active`, iconos, CSP) y dirección del servidor configurable
  desde la pantalla de acceso.
- Documentación: despliegue, operación, seguridad, arquitectura, modelo, sincronización y pruebas.

Criterio de salida: criterios 11–12. **Parcial**: la suite y las migraciones están verificadas; el
arranque con Docker queda pendiente de un equipo con Docker Engine.

## Criterios de aceptación (seguimiento)

| # | Criterio | Estado actual |
|---|---|---|
| 1 | Pedido de 5: 2 entregados, 1 descartado, 2 pendientes → PARCIAL | Cumple (verificado por HTTP) |
| 2 | Entregar los 2 restantes → COMPLETADO 4+1 | Cumple (verificado por HTTP) |
| 3 | Custodia solo tras acuse autenticado | Cumple: el receptor designado confirma con su sesión y se registra `acuses` |
| 4 | Dos receptores simultáneos → una custodia y un conflicto | Cumple: doble confirmación y doble aceptación de transferencia devuelven 200/409 con una sola custodia |
| 5 | Descartar no borra expediente ni historial | Cumple |
| 6 | Devolución solo con acuse del almacén; transferencia con acuse | **Cumple**: flujos en dos pasos verificados |
| 7 | Pedido offline se sincroniza una vez | Cumple: outbox con clave única y push idempotente |
| 8 | Cliente offline no confirma custodia | Cumple: el motor solo encola `CREAR_PEDIDO`; el servidor rechaza el resto |
| 9 | Usuario sin permisos no opera | **Cumple** (RBAC por permisos, probado) |
| 10 | Reinicio no pierde outbox ni duplica movimientos | Cumple: outbox persistente en SQLite + idempotencia en servidor |
| 11 | Migraciones, compilación y suite automatizada | Parcial: 52 pruebas de servidor y 8 del cliente en verde; falta ejecutar Docker y la prueba E2E de escritorio |
| 12 | README reproducible sin secretos | Cumple en documentación: instalación, demo, despliegue y operación; falta validarlo en el servidor Debian |

