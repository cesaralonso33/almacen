CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE TABLE users (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 name text NOT NULL
);
CREATE TABLE expedientes (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 numero text NOT NULL UNIQUE,
 descripcion text NOT NULL DEFAULT '',
 estado text NOT NULL DEFAULT 'ALMACEN' CHECK (estado IN ('ALMACEN','EN_CUSTODIA','NO_LOCALIZADO')),
 custodio_id uuid REFERENCES users(id),
 version bigint NOT NULL DEFAULT 0,
 CHECK ((estado='EN_CUSTODIA') = (custodio_id IS NOT NULL))
);
CREATE TABLE pedidos (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 solicitante_id uuid NOT NULL REFERENCES users(id),
 creado_en timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE pedido_items (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 pedido_id uuid NOT NULL REFERENCES pedidos(id),
 expediente_id uuid NOT NULL REFERENCES expedientes(id),
 estado text NOT NULL DEFAULT 'PENDIENTE' CHECK (estado IN ('PENDIENTE','PREPARADO','EN_ENTREGA','ENTREGADO','DESCARTADO')),
 motivo_descarte text,
 UNIQUE (pedido_id, expediente_id)
);
CREATE TABLE entregas (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 pedido_item_id uuid NOT NULL UNIQUE REFERENCES pedido_items(id),
 entregador_id uuid NOT NULL REFERENCES users(id),
 receptor_id uuid NOT NULL REFERENCES users(id),
 estado text NOT NULL DEFAULT 'PENDIENTE_RECEPCION' CHECK (estado IN ('PENDIENTE_RECEPCION','CONFIRMADA')),
 creada_en timestamptz NOT NULL DEFAULT now(),
 confirmada_en timestamptz
);
CREATE TABLE custodias (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 expediente_id uuid NOT NULL REFERENCES expedientes(id),
 usuario_id uuid NOT NULL REFERENCES users(id),
 entrega_id uuid REFERENCES entregas(id),
 inicio timestamptz NOT NULL DEFAULT now(),
 fin timestamptz
);
CREATE UNIQUE INDEX una_custodia_activa ON custodias(expediente_id) WHERE fin IS NULL;
CREATE TABLE movimientos (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 expediente_id uuid NOT NULL REFERENCES expedientes(id),
 tipo text NOT NULL,
 actor_id uuid NOT NULL REFERENCES users(id),
 destino_id uuid REFERENCES users(id),
 detalle text,
 creado_en timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE processed_commands (
 id uuid PRIMARY KEY,
 response jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX pedidos_por_solicitante ON pedidos(solicitante_id, creado_en DESC);
CREATE INDEX items_por_pedido ON pedido_items(pedido_id);
CREATE INDEX movimientos_por_expediente ON movimientos(expediente_id, creado_en DESC);
