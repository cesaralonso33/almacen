# Sincronización offline

## Principios

1. El servidor es la única fuente de verdad para custodia, entregas, devoluciones y permisos.
2. El cliente puede leer datos autorizados previamente sincronizados y crear pedidos en cola.
3. Toda operación local lleva una **clave UUID**; el servidor es idempotente por (clave, dispositivo).
4. El cursor local avanza solo en la misma transacción que aplica los cambios.
5. Un rechazo del servidor se conserva como conflicto visible y **no se reintenta solo**.

## Servidor

- `sync_events` con secuencia global monotónica, `tombstone`, `version` y visibilidad:
  - `GLOBAL`: inventario y custodia (todo usuario autenticado).
  - `ROL`: eventos de almacén (`ALMACEN`).
  - `USUARIO`: pedidos del solicitante.
- `POST /sync/push`: valida dispositivo, aplica cada operación en su propia transacción y guarda
  el resultado en `operaciones_idempotentes`. Devuelve `ACEPTADA` o `RECHAZADA` por operación.
- `GET /sync/pull?cursor=`: eventos posteriores al cursor, filtrados por rol y usuario.
  Si el cursor es mayor que la secuencia máxima devuelve `reiniciar: true`.
- `GET /sync/snapshot`: expedientes autorizados, pedidos propios con renglones y cursor.

Solo se acepta `CREAR_PEDIDO` sin conexión. Confirmar custodia, entrega, devolución o permisos
devuelve `RECHAZADA`.

## Escritorio

| Tabla local | Contenido |
|---|---|
| `expedientes`, `pedidos`, `pedido_items`, `entregas` | Proyecciones autorizadas |
| `outbox` | Operaciones pendientes con clave, intentos y próximo intento |
| `sync_metadata` | Cursor, dispositivo y servidor configurado |
| `sync_conflicts` | Rechazos pendientes de revisión humana |
| `sync_status` | Estado, último error, última sincronización y pendientes |

- Retroceso exponencial con jitter: 5 s, 10 s, 20 s… hasta 10 minutos.
- Si el push falla por red, la operación permanece `PENDIENTE` y se reintenta.
- Si el servidor responde `RECHAZADA`, la operación pasa a `RECHAZADA` y se crea un conflicto.

## Flujo típico sin conexión

1. La persona busca expedientes: se usa la copia local con aviso de desactualización.
2. Crea un pedido: queda `PENDIENTE_DE_SINCRONIZAR` y en el outbox.
3. Al reconectar, «Sincronizar» empuja la operación; si se repite, el servidor devuelve el mismo
   resultado sin duplicar.
4. El pull trae los cambios de estado y el pedido local se actualiza.

## Pruebas

- Servidor (9): snapshot, orden y paginación, cursor inválido, filtrado por usuario, idempotencia,
  duplicado concurrente, operación no permitida, dispositivo ajeno.
- Escritorio (8): snapshot, lote inválido sin avanzar cursor, custodia, rechazo con conflicto,
  reintento tras error, ciclo pull+push, reconstrucción por cursor inválido, outbox persistente.
