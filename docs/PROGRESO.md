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

## Estado frente a las fases

- Fase 0: **completada** (este documento y `docs/PLAN.md`).
- Fase 1: **parcial**. Modelo institucional y pruebas de invariantes listos; faltan semillas de
  usuarios, alineación de estados y uso de reservas desde la API.
- Fases 2–6: no iniciadas.

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
