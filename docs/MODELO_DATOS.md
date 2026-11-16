# Modelo de datos

Migraciones en `server/migrations` (SQLx, aplicadas al iniciar la API).

## Tablas principales

| Tabla | Propósito | Restricciones clave |
|---|---|---|
| `usuarios` | Cuentas | correo único sin distinguir mayúsculas |
| `roles`, `permisos`, `rol_permisos`, `usuario_roles` | RBAC | una asignación activa por usuario y rol |
| `dispositivos`, `sesiones` | Equipos y refresh tokens | hash de refresh único; revocación |
| `ubicaciones` | EDIFICIO → AREA → ESTANTE → CAJA | trigger de jerarquía; código único por padre |
| `expedientes` | Inventario físico | número único global; versión |
| `pedidos`, `pedido_items` | Solicitudes | un expediente por pedido |
| `reservas` | Bloqueo temporal de preparación | una activa por expediente; vencimiento |
| `entregas`, `acuses` | Despacho y acuse | un acuse por entrega |
| `custodias` | Custodia oficial | **una activa por expediente** (índice parcial) |
| `descartes`, `motivos_descarte` | No atención de renglones | motivo catalogado; `OTRO` exige detalle |
| `devoluciones` | Devolución en dos pasos | una solicitud activa por expediente |
| `transferencias` | Cambio directo de custodio | una solicitud activa por expediente |
| `movimientos` | Historial operativo por expediente | — |
| `auditoria` | Bitácora | append-only por reglas |
| `sync_events`, `sync_devices` | Sincronización | secuencia monotónica; inmutables |
| `operaciones_idempotentes` | Reintentos seguros | PK (clave, dispositivo) |

## Estados

- **Pedido** (derivado): `PENDIENTE`, `EN_PROCESO`, `PARCIAL`, `COMPLETADO`, `CANCELADO`.
- **Renglón**: `PENDIENTE`, `RESERVADO`, `PREPARADO`, `EN_ENTREGA`, `ENTREGADO`, `DESCARTADO`,
  `CANCELADO`.
- **Entrega**: `PREPARADA`, `DESPACHADA`, `RECIBIDA`, `CANCELADA`, `INCIDENCIA`.
- **Expediente**: `ALMACEN`, `EN_CUSTODIA`, `NO_LOCALIZADO`.
- **Devolución**: `SOLICITADA`, `CONFIRMADA`, `CANCELADA`.
- **Transferencia**: `SOLICITADA`, `ACEPTADA`, `RECHAZADA`, `CANCELADA`.

## Invariantes

1. Una sola custodia activa por expediente.
2. Una sola reserva activa por expediente y una sola devolución o transferencia en curso.
3. La custodia cambia solo con acuse autenticado (recepción, devolución o transferencia aceptada).
4. Un descarte no borra el expediente ni el historial.
5. La auditoría y los eventos de sincronización no se modifican ni se borran.
6. El pedido no puede cancelarse con renglones entregados o en tránsito.
7. Una transferencia exige origen y destino distintos.
8. Al registrar un expediente, el creador queda como custodio inicial; solo lo pierde con una
   devolución confirmada por el almacén o una transferencia aceptada.
9. El número se normaliza a 8 caracteres antes de la primera barra, rellenando con ceros a la
   izquierda; la unicidad es global y se evalúa sobre el valor normalizado.
