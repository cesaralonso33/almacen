# AGENTS.md — Instrucciones maestras para OpenCode

## Misión
Construye, integra, prueba y documenta una aplicación de escritorio **lista para despliegue institucional** para el almacén, solicitudes, entrega, devolución, transferencia y cadena de custodia de **expedientes jurídicos físicos**. Trabaja sobre el código existente de este repositorio; no te limites a producir planes o pseudocódigo. No afirmes que está terminada sin evidencias de compilación, pruebas y flujos funcionales.

Idioma de interfaz, mensajes, documentación y datos de demostración: **español (México)**. Zona horaria de visualización configurable, predeterminada `America/Mexico_City`; almacenar instantes en UTC. Diseño minimalista, profesional, rápido y accesible.

## Contexto y decisiones innegociables
- Aproximadamente 20 usuarios, ubicados en diferentes edificios; un expediente puede pasar entre usuarios sin secuencia fija.
- Servidor **local institucional Debian** accesible por LAN institucional o VPN; no requiere internet público.
- Escritorio **Tauri 2 + Svelte 5 + TypeScript + Vite + Tailwind CSS**, Rust para comandos locales, **SQLite** para cache y outbox offline.
- API central **Rust + Axum + SQLx**, base oficial **PostgreSQL**. Nunca conectar los clientes directamente a PostgreSQL.
- Autenticación, autorización en servidor, bitácora auditable, aislamiento por rol y principio de mínimo privilegio.
- Soportar **entrega parcial y completa**: cada pedido contiene múltiples renglones; cada renglón se resuelve independientemente. El pedido se completa cuando todos sus renglones estén entregados o descartados; mostrar si fue atendido completamente, parcialmente o con descartes. No confundir completar trámite con entregar todos los expedientes.
- Un descarte significa **no atender el renglón del pedido**, con motivo obligatorio y usuario responsable; **no borrar** ni dar de baja el expediente. Registrar motivos: no localizado, no disponible, en custodia de otro usuario, registro erróneo y otro con descripción.
- Custodia oficial **solo** después de acuse de recepción autenticado. En entregas pendientes de acuse, mantener custodio oficial anterior y marcar transferencia en tránsito. Las devoluciones requieren acuse del almacén. Transferencias directas entre usuarios requieren acuse del receptor.
- Una única custodia oficial activa por expediente, con restricciones e invariantes en base de datos; evitar dobles reservas y despachos simultáneos mediante transacciones y bloqueos de filas.
- Offline: permitir consultas de datos autorizados previamente sincronizados, borradores y creación de pedidos en cola. **No confirmar unilateralmente custodia, entregas, devoluciones ni permisos sin servidor**. Permitir registrar contingencia offline como evidencia local no oficial pendiente de conciliación, claramente diferenciada.
- No usar códigos QR como requisito; identificación por número de expediente y búsqueda.
- No almacenar secretos en repositorio; TLS para LAN/VPN, credenciales seguras, cifrado de datos locales sensibles, backups y recuperación.

## Inspección inicial obligatoria
1. Lee `README.md`, `server/`, `desktop/`, `infra/` y migraciones existentes. Identifica qué funciona y qué es prototipo.
2. Ejecuta las comprobaciones que el entorno permita (`cargo check`, `cargo test`, `npm install`, `npm run check`, `npm run build`, etc.). Registra fallos previos antes de editar.
3. Mantén las rutas existentes cuando sea razonable, pero refactoriza si se justifica. No asumas que los endpoints actuales son seguros: el starter acepta `actor_id` suministrado por el cliente y **debe reemplazarse** por identidad de sesión autenticada.
4. Crea `docs/PLAN.md` con fases, dependencias y criterios de aceptación; mantén `docs/PROGRESO.md` actualizado con estado real, comandos ejecutados y problemas.
5. Trabaja de forma autónoma fase por fase. No pidas confirmación para decisiones técnicas menores; documenta las decisiones. Pregunta solo por una decisión institucional imprescindible que bloquee el trabajo.

## Estructura objetivo
```
expedientes-starter/
  AGENTS.md
  README.md
  docs/{PLAN,PROGRESO,ARQUITECTURA,MODELO_DATOS,SEGURIDAD,SINCRONIZACION,DESPLIEGUE,OPERACION,PRUEBAS}.md
  server/
    Cargo.toml
    migrations/*.sql
    src/{main.rs,config.rs,auth/,api/,domain/,db/,sync/,audit/,errors/}
    tests/
  desktop/
    package.json
    src/{App.svelte,main.ts,lib/{api,components,stores,types,utils},routes/}
    src-tauri/{Cargo.toml,src/{commands,db,sync,security},migrations/,capabilities/}
  infra/{docker-compose.yml,Caddyfile,.env.example,backup/,restore/}
  scripts/
```
Puedes elegir una estructura idiomática mejor, manteniendo separación clara entre dominio, persistencia, API, UI y sincronización.

## Modelo de datos PostgreSQL
Crear migraciones SQLx versionadas, reproducibles, reversibles cuando proceda, con índices, FKs y restricciones. Entidades mínimas:
- `usuarios`, `roles`, `permisos`, `usuario_roles`, `sesiones`/tokens revocables, `dispositivos`.
- `ubicaciones` jerárquicas (edificio, área, estante, caja), `expedientes` (UUID, número, año, unidad/órgano, descripción autorizada, ubicación, estado físico, versión, fechas), catálogos de estados.
- `pedidos`, `pedido_detalles`, `reservas` con vencimiento configurable, `entregas`, `entrega_detalles`, `acuses`, `custodias`, `movimientos`, `incidencias`, `descartes`.
- `auditoria` append-only, `operaciones_idempotentes`, `sync_events` con secuencia global monotónica, `sync_devices`/cursores y controles de visibilidad.
- La unicidad de número de expediente debe parametrizarse según unidad/año; no asumir unicidad universal sin documentación.
- Evitar guardar estados derivados contradictorios; si se desnormalizan por rendimiento, mantenerlos transaccionalmente.
- `custodias` con índice único parcial `WHERE fin IS NULL` por expediente, y cambios atómicos en transacción.
- `sync_events` debe incluir borrados lógicos/tombstones y versiones; respetar filtrado por permisos.
- Añadir semillas de desarrollo con cuentas y expedientes ficticios, nunca credenciales reales; contraseñas de demo claramente documentadas y obligar cambio antes de producción.

## Estados y reglas de negocio
Pedido: `PENDIENTE`, `EN_PROCESO`, `PARCIAL`, `COMPLETADO`, `CANCELADO` (derivados de renglones, evitando inconsistencias).
Renglón: `PENDIENTE`, `RESERVADO`, `PREPARADO`, `EN_ENTREGA`, `ENTREGADO`, `DESCARTADO`, `CANCELADO`.
Entrega: `PREPARADA`, `DESPACHADA`, `RECIBIDA`, `CANCELADA`, `INCIDENCIA`.
Expediente: distinguir existencia/ubicación de disponibilidad y custodia; un expediente `NO_LOCALIZADO` no debe ser entregable sin regularización autorizada.

Flujo pedido: usuario solicita N expedientes -> almacén revisa cada renglón -> reserva/prepara o descarta con motivo -> despacha subconjunto -> receptor confirma recepción -> custodia cambia -> pedido recalcula progreso. Permitir repetir entregas para renglones restantes. Impedir entregar renglón descartado o ya entregado.
Flujo devolución: custodio solicita devolución -> almacén recibe y confirma -> se cierra custodia y actualiza ubicación.
Flujo transferencia: custodio solicita transferencia -> receptor acepta -> se cierra custodia anterior y abre la nueva en una transacción.
Si se pierde conexión durante confirmación, reintentar con clave de idempotencia; jamás crear doble movimiento.
Cada acción importante debe registrar actor desde token, fecha servidor, expediente, pedido/renglón, dispositivo, origen/destino, observaciones, resultado e ID de correlación.

## API y seguridad
API JSON `/api/v1`; definir contratos OpenAPI y validación de entrada. Rutas mínimas:
- auth: login, refresh, logout, me; administración de usuarios y roles.
- expedientes: CRUD autorizado, búsqueda, filtros, historial, ubicación.
- pedidos: crear con múltiples IDs, listar/filtrar, detalle, cancelar con reglas, preparar/reservar renglón, descartar con motivo.
- entregas: crear con subconjunto de renglones, despachar, confirmar recepción, consultar acuse.
- devoluciones y transferencias con doble confirmación.
- dashboard y reportes; sync push/pull; health/readiness; eventos WebSocket o SSE como señal de refresco (no fuente única de verdad).
- Todas las mutaciones sensibles requieren autenticación, permisos y comprobación de estado. No aceptar `actor_id` como identidad autoritativa.
- Password hashing Argon2id; cookies seguras o tokens de acceso de corta vida + refresh rotativo/revocable; rate limiting; CORS estricto si aplica; no registrar secretos ni datos confidenciales en logs.
- Control de acceso también en pull offline: no sincronizar indiscriminadamente expedientes reservados a otras áreas. Revocaciones deben eliminar/invalidar cache sensible cuando sea posible.
- Registrar y probar respuestas 400, 401, 403, 404, 409, 422, 429, 500 con errores consistentes.

## Sincronización offline
SQLite local por perfil/dispositivo, con tablas `outbox`, `sync_metadata`, proyecciones autorizadas de expedientes/pedidos, `sync_conflicts`, `sync_status`. Migraciones locales versionadas.
- Escritura local de operación y UI optimista en transacción SQLite, mostrando `PENDIENTE_DE_SINCRONIZAR` sin confundir con estado oficial.
- Outbox con UUID de operación, `device_id`, secuencia local, tipo, payload, versión base, estado, intentos y error; reintentos exponenciales con jitter y límite.
- `POST /sync/push`: idempotencia persistida, procesar operaciones autorizadas, devolver por operación aceptación/rechazo/conflicto y versión oficial.
- `GET /sync/pull?cursor=...`: eventos incrementales ordenados, paginados, filtrados por permisos, con cursores seguros; snapshots iniciales y recuperación cuando el cursor ya no sea válido.
- Aplicar lote de cambios y cursor en **una transacción SQLite**; no adelantar cursor antes de persistir.
- Resolución: pedidos offline se aceptan si cumplen reglas; conflictos de custodia/reserva no se sobrescriben; mostrar bandeja de incidencias para resolución humana.
- WebSocket/SSE solo notifica cambios; el pull incremental recupera eventos perdidos.
- Sesiones offline con vigencia acotada; no conceder privilegios nuevos offline.
- Pruebas: duplicados, cortes a mitad de push/pull, reconexión, dos clientes concurrentes, permisos revocados, cambios fuera de orden, entrega simultánea.

## Interfaz Svelte 5
- Navegación lateral compacta: Inicio, Expedientes, Mis pedidos, Almacén, Entregas, Mis custodias, Devoluciones, Movimientos, Reportes, Administración.
- Panel con contadores: pedidos pendientes, por preparar, en entrega, custodias activas, incidencias y no localizados.
- Tabla de expedientes con búsqueda rápida, filtros por columna, orden, paginación/virtualización, columnas configurables, exportación CSV autorizada y detalle.
- Formulario para pedidos múltiples con búsqueda y selección; bandeja de almacén con preparación y descarte individual; selección de renglones para entrega parcial o completa; acuse de recepción explícito.
- Panel de custodias del usuario, transferencias y devoluciones; historial temporal por expediente; reportes.
- Estado visible `En línea`, `Sin conexión`, `Sincronizando`, `Pendiente`, `Conflicto`, `Error`; distinguir información local potencialmente desactualizada.
- Confirmaciones para acciones irreversibles, validación, estados vacíos, skeletons, navegación por teclado, contrastes y responsive para resoluciones pequeñas.
- Priorizar velocidad: caché SQLite, índices, virtualización, debounce, consultas asíncronas, no bloquear UI; evitar refrescos completos.

## Despliegue institucional
- Docker Compose para PostgreSQL, API y proxy Caddy; `.env.example` sin secretos; healthchecks y reinicio; PostgreSQL solo en red privada de contenedores.
- TLS con CA institucional o certificados confiables; documentar instalación en clientes y DNS interno.
- Scripts de migración, respaldo PostgreSQL, restauración probada, retención, permisos de archivos, monitoreo y rotación de logs.
- Paquetes Tauri para plataforma objetivo, instrucciones de instalación/actualización y configuración del endpoint local; no incrustar contraseñas en instalador.
- Documentar funcionamiento LAN sin internet, VPN entre edificios y qué ocurre si se cae el servidor.

## Plan de ejecución obligatorio
**Fase 0** Diagnóstico y plan; identificar riesgos del starter.
**Fase 1** Modelo PostgreSQL, migraciones, semillas, restricciones, tests de DB.
**Fase 2** Autenticación y RBAC; eliminar `actor_id` confiado del cliente.
**Fase 3** Pedidos múltiples, descartes, reservas, entregas parciales/completas, acuses, custodia, devoluciones, transferencias, auditoría; tests de concurrencia.
**Fase 4** API de sincronización, idempotencia, eventos, SQLite y motor de sync Tauri; tests offline.
**Fase 5** UI Svelte completa integrada con API y cache; pruebas de flujos.
**Fase 6** Despliegue Debian, TLS, backups, seguridad, optimización y documentación.

En cada fase: implementar -> ejecutar compilación/lint/tests -> corregir -> actualizar `docs/PROGRESO.md` -> continuar. Si faltan herramientas del entorno, anotar comando exacto y limitación; no inventar resultados.

## Criterios de aceptación (todos obligatorios)
1. Un usuario autenticado crea un pedido con 5 expedientes; almacén entrega 2, descarta 1 con motivo y deja 2 pendientes; el pedido queda parcial con conteos correctos.
2. Se entregan los 2 restantes; el pedido queda completado y refleja 4 entregados + 1 descartado.
3. No se cambia la custodia hasta el acuse autenticado del receptor.
4. Dos usuarios intentan recibir el mismo expediente: solo una custodia activa; la otra operación devuelve conflicto.
5. Descartar pedido no borra expediente ni historial.
6. Devolución solo finaliza cuando almacén confirma recepción; transferencia solo con aceptación del receptor.
7. Cliente sin red crea pedido; UI lo marca pendiente; al reconectar se sincroniza una vez aunque se reintente.
8. Cliente desconectado no puede confirmar custodia oficial.
9. Un usuario sin permisos no puede preparar, descartar, entregar ni consultar datos fuera de su ámbito.
10. Reiniciar servidor y clientes no pierde operaciones en outbox ni duplica movimientos.
11. Migraciones desde base vacía, compilación API/escritorio y suite automatizada pasan.
12. README permite reproducir instalación, demo y despliegue; sin secretos hardcodeados.

## Estilo de trabajo de OpenCode
- Haz cambios reales en archivos. Prioriza funcionalidad end-to-end antes que interfaces ficticias.
- No sustituyas implementación por mocks permanentes. Los mocks de pruebas deben estar aislados.
- Si hay errores de diseño del starter, corrígelos aunque implique migraciones nuevas y cambios de contrato.
- Usa versiones de dependencias compatibles, verificadas por compilación; no inventes APIs.
- Usa transacciones SQLx y tests de integración para invariantes de custodia; nunca dependas solo de validación del frontend.
- Mantén documentación actualizada y reporta qué funciona, qué falta, cómo probar y riesgos abiertos.
- No elimines datos ni ejecutes migraciones destructivas en bases ajenas sin copia de seguridad y autorización explícita.
- Cuando completes todas las fases, entrega resumen de archivos, comandos, pruebas verificadas y pasos de despliegue.

## Primera acción ahora
Inspecciona el repositorio, ejecuta diagnóstico, escribe `docs/PLAN.md` y `docs/PROGRESO.md`, y comienza **de inmediato** con migraciones y backend seguro de pedidos, descartes, entregas parciales/completas y cadena de custodia. Continúa con las demás fases hasta satisfacer los criterios de aceptación.
