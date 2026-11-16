-- 0004: base de sincronización incremental
-- Versión de pedido para cursores y eventos, e índices de filtrado por permisos.

ALTER TABLE pedidos ADD COLUMN version bigint NOT NULL DEFAULT 0;

CREATE INDEX sync_events_por_usuario ON sync_events (usuario_id, secuencia)
    WHERE visibilidad = 'USUARIO';

-- Los eventos de sincronización son inmutables: son la bitácora de cambios para los clientes.
CREATE RULE sync_events_no_update AS ON UPDATE TO sync_events DO INSTEAD NOTHING;
CREATE RULE sync_events_no_delete AS ON DELETE TO sync_events DO INSTEAD NOTHING;
