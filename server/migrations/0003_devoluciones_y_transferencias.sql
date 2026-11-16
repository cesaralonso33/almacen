-- 0003: devoluciones con acuse del almacén, transferencias con acuse del receptor
-- y estados operativos alineados con el flujo institucional.

-- ---------------------------------------------------------------------------
-- Estados de renglón: se añaden RESERVADO y CANCELADO
-- ---------------------------------------------------------------------------
ALTER TABLE pedido_items DROP CONSTRAINT pedido_items_estado_check;
ALTER TABLE pedido_items ADD CONSTRAINT pedido_items_estado_check
    CHECK (estado IN ('PENDIENTE', 'RESERVADO', 'PREPARADO', 'EN_ENTREGA', 'ENTREGADO', 'DESCARTADO', 'CANCELADO'));
ALTER TABLE pedido_items ADD COLUMN cancelado_motivo text;

-- ---------------------------------------------------------------------------
-- Estados de entrega: PREPARADA, DESPACHADA, RECIBIDA, CANCELADA, INCIDENCIA
-- ---------------------------------------------------------------------------
ALTER TABLE entregas DROP CONSTRAINT entregas_estado_check;
UPDATE entregas SET estado = CASE estado
    WHEN 'PENDIENTE_RECEPCION' THEN 'DESPACHADA'
    WHEN 'CONFIRMADA' THEN 'RECIBIDA'
    ELSE estado
END;
ALTER TABLE entregas ALTER COLUMN estado SET DEFAULT 'DESPACHADA';
ALTER TABLE entregas ADD CONSTRAINT entregas_estado_check
    CHECK (estado IN ('PREPARADA', 'DESPACHADA', 'RECIBIDA', 'CANCELADA', 'INCIDENCIA'));

-- ---------------------------------------------------------------------------
-- Cancelación de pedidos
-- ---------------------------------------------------------------------------
ALTER TABLE pedidos
    ADD COLUMN cancelado_en timestamptz,
    ADD COLUMN cancelado_por uuid REFERENCES usuarios(id),
    ADD COLUMN cancelado_motivo text;

ALTER TABLE pedidos ADD CONSTRAINT pedidos_cancelacion_coherente
    CHECK ((cancelado_en IS NULL) = (cancelado_motivo IS NULL));

-- ---------------------------------------------------------------------------
-- Devoluciones: el custodio solicita, el almacén acusa la recepción
-- ---------------------------------------------------------------------------
CREATE TABLE devoluciones (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    expediente_id uuid NOT NULL REFERENCES expedientes(id),
    custodio_id uuid NOT NULL REFERENCES usuarios(id),
    estado text NOT NULL DEFAULT 'SOLICITADA'
        CHECK (estado IN ('SOLICITADA', 'CONFIRMADA', 'CANCELADA')),
    solicitada_en timestamptz NOT NULL DEFAULT now(),
    recibida_por uuid REFERENCES usuarios(id),
    confirmada_en timestamptz,
    ubicacion_destino_id uuid REFERENCES ubicaciones(id) ON DELETE RESTRICT,
    observaciones text,
    CHECK ((estado = 'CONFIRMADA') = (confirmada_en IS NOT NULL))
);

-- Una sola devolución en curso por expediente.
CREATE UNIQUE INDEX devoluciones_activas ON devoluciones (expediente_id)
    WHERE estado = 'SOLICITADA';

CREATE INDEX devoluciones_por_custodio ON devoluciones (custodio_id, solicitada_en DESC);

-- ---------------------------------------------------------------------------
-- Transferencias directas entre usuarios: el receptor debe aceptar
-- ---------------------------------------------------------------------------
CREATE TABLE transferencias (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    expediente_id uuid NOT NULL REFERENCES expedientes(id),
    origen_id uuid NOT NULL REFERENCES usuarios(id),
    destino_id uuid NOT NULL REFERENCES usuarios(id),
    estado text NOT NULL DEFAULT 'SOLICITADA'
        CHECK (estado IN ('SOLICITADA', 'ACEPTADA', 'RECHAZADA', 'CANCELADA')),
    solicitada_en timestamptz NOT NULL DEFAULT now(),
    resuelta_en timestamptz,
    observaciones text,
    CHECK (origen_id <> destino_id),
    CHECK ((estado IN ('ACEPTADA', 'RECHAZADA', 'CANCELADA')) = (resuelta_en IS NOT NULL))
);

-- Una sola transferencia en curso por expediente.
CREATE UNIQUE INDEX transferencias_activas ON transferencias (expediente_id)
    WHERE estado = 'SOLICITADA';

CREATE INDEX transferencias_por_destino ON transferencias (destino_id, solicitada_en DESC);

-- ---------------------------------------------------------------------------
-- Permiso para solicitar transferencias
-- ---------------------------------------------------------------------------
INSERT INTO permisos (clave, descripcion)
VALUES ('transferencias:solicitar', 'Solicitar la transferencia de custodia de un expediente')
ON CONFLICT (clave) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave = 'transferencias:solicitar'
WHERE r.clave IN ('ADMINISTRADOR', 'ALMACEN', 'USUARIO')
ON CONFLICT DO NOTHING;
