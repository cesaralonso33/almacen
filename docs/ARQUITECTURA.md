# Arquitectura

## Componentes

```
Escritorio (Tauri 2 + Svelte 5 + SQLite)
    │  HTTPS (solo API)
    ▼
Caddy (TLS, proxy inverso)
    ▼
API Rust + Axum ─── PostgreSQL 17
    │
    └─ sync_events (push/pull incremental)
```

- **Escritorio**: interfaz, caché autorizada, outbox y motor de sincronización. Nunca accede a
  PostgreSQL.
- **API**: autenticación, RBAC, reglas de negocio, transacciones, auditoría y eventos.
- **PostgreSQL**: fuente única de verdad con restricciones e índices.
- **Caddy**: TLS institucional y cabeceras de seguridad.

## Módulos del servidor

| Módulo | Responsabilidad |
|---|---|
| `auth/` | Argon2id, JWT, extractor `AuthUser` con permisos |
| `api/auth.rs` | login, refresh, logout, cambio de contraseña, me |
| `api/admin.rs` | usuarios y roles |
| `api/pedidos.rs` | alta, detalle, preparación, descarte, cancelación |
| `api/entregas.rs` | despacho y acuse de recepción |
| `api/devoluciones.rs` | solicitud y acuse del almacén |
| `api/transferencias.rs` | solicitud y aceptación/rechazo |
| `api/consultas.rs` | tablero, listados e historial |
| `api/sync.rs` | dispositivos, push, pull y snapshot |
| `sync.rs` | emisión de eventos |
| `audit.rs` | bitácora transaccional |

## Flujo de un pedido

1. El solicitante crea el pedido, con una reserva por renglón.
2. Almacén prepara o descarta cada renglón; el descarte exige motivo catalogado.
3. El despacho crea la entrega y deja el renglón `EN_ENTREGA`; **la custodia no cambia**.
4. El receptor designado confirma con su sesión: se crea el acuse, se cierra el renglón y se abre
   la custodia oficial.
5. El pedido recalcula su estado: `PENDIENTE`, `EN_PROCESO`, `PARCIAL`, `COMPLETADO`, `CANCELADO`.

## Custodia

- Índice único parcial `custodias(expediente_id) WHERE fin IS NULL`: una sola custodia activa.
- Orden de bloqueo global: **expediente → renglón/entrega**. Evita interbloqueos entre preparación,
  despacho, acuse, devolución y transferencia.
- Devolución: la custodia se cierra solo con el acuse del almacén.
- Transferencia: se cierra la anterior y se abre la nueva en una sola transacción, solo al aceptar.

## Sincronización

- Cada operación de negocio inserta `sync_events` en su transacción.
- Push idempotente por (clave, dispositivo); pull incremental por cursor y filtrado por permisos.
- El escritorio aplica lote + cursor en una transacción SQLite.
- Rechazos se convierten en `sync_conflicts` para revisión humana; nunca se reenvían solos.
