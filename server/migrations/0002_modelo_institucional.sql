-- 0002: modelo institucional mínimo
-- RBAC, sesiones/dispositivos, ubicaciones jerárquicas, reservas, descartes catalogados,
-- acuses, incidencias, auditoría append-only y base de sincronización.
-- Aditivo sobre 0001; no elimina datos.

-- ---------------------------------------------------------------------------
-- Usuarios
-- ---------------------------------------------------------------------------
ALTER TABLE users RENAME TO usuarios;
ALTER TABLE usuarios RENAME COLUMN name TO nombre;

ALTER TABLE usuarios
    ADD COLUMN correo text,
    ADD COLUMN password_hash text,
    ADD COLUMN activo boolean NOT NULL DEFAULT true,
    ADD COLUMN debe_cambiar_password boolean NOT NULL DEFAULT true,
    ADD COLUMN creado_en timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN actualizado_en timestamptz NOT NULL DEFAULT now();

CREATE UNIQUE INDEX usuarios_correo_unico ON usuarios (lower(correo)) WHERE correo IS NOT NULL;

-- ---------------------------------------------------------------------------
-- RBAC
-- ---------------------------------------------------------------------------
CREATE TABLE roles (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    clave text NOT NULL UNIQUE,
    nombre text NOT NULL,
    descripcion text,
    creado_en timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE permisos (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    clave text NOT NULL UNIQUE,
    descripcion text NOT NULL,
    creado_en timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE rol_permisos (
    rol_id uuid NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permiso_id uuid NOT NULL REFERENCES permisos(id) ON DELETE CASCADE,
    PRIMARY KEY (rol_id, permiso_id)
);

CREATE TABLE usuario_roles (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    usuario_id uuid NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE,
    rol_id uuid NOT NULL REFERENCES roles(id) ON DELETE RESTRICT,
    asignado_en timestamptz NOT NULL DEFAULT now(),
    revocado_en timestamptz
);

-- Un usuario no puede acumular dos asignaciones activas del mismo rol.
CREATE UNIQUE INDEX usuario_roles_activo ON usuario_roles (usuario_id, rol_id)
    WHERE revocado_en IS NULL;

-- ---------------------------------------------------------------------------
-- Dispositivos y sesiones
-- ---------------------------------------------------------------------------
CREATE TABLE dispositivos (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    usuario_id uuid REFERENCES usuarios(id) ON DELETE CASCADE,
    nombre text NOT NULL,
    huella text,
    plataforma text,
    registrado_en timestamptz NOT NULL DEFAULT now(),
    ultimo_uso_en timestamptz,
    revocado_en timestamptz
);

CREATE INDEX dispositivos_por_usuario ON dispositivos (usuario_id) WHERE revocado_en IS NULL;

CREATE TABLE sesiones (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    usuario_id uuid NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE,
    dispositivo_id uuid REFERENCES dispositivos(id) ON DELETE SET NULL,
    refresh_token_hash text NOT NULL UNIQUE,
    creada_en timestamptz NOT NULL DEFAULT now(),
    expira_en timestamptz NOT NULL,
    revocada_en timestamptz,
    reemplazada_por uuid REFERENCES sesiones(id),
    ip inet,
    user_agent text,
    CHECK (expira_en > creada_en)
);

CREATE INDEX sesiones_por_usuario ON sesiones (usuario_id, creada_en DESC);
CREATE INDEX sesiones_activas ON sesiones (usuario_id) WHERE revocada_en IS NULL;

-- ---------------------------------------------------------------------------
-- Ubicaciones jerárquicas: EDIFICIO > AREA > ESTANTE > CAJA
-- ---------------------------------------------------------------------------
CREATE TABLE ubicaciones (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    padre_id uuid REFERENCES ubicaciones(id) ON DELETE RESTRICT,
    tipo text NOT NULL CHECK (tipo IN ('EDIFICIO', 'AREA', 'ESTANTE', 'CAJA')),
    codigo text NOT NULL CHECK (length(trim(codigo)) > 0),
    nombre text NOT NULL,
    activa boolean NOT NULL DEFAULT true,
    creada_en timestamptz NOT NULL DEFAULT now()
);

-- El índice debe cubrir también el nivel raíz (padre_id NULL).
CREATE UNIQUE INDEX ubicaciones_codigo_unico
    ON ubicaciones (COALESCE(padre_id, '00000000-0000-0000-0000-000000000000'::uuid), codigo);

CREATE OR REPLACE FUNCTION validar_jerarquia_ubicacion() RETURNS trigger AS $$
DECLARE
    tipo_padre text;
BEGIN
    IF NEW.padre_id IS NULL THEN
        IF NEW.tipo <> 'EDIFICIO' THEN
            RAISE EXCEPTION 'solo un EDIFICIO puede ser ubicación raíz';
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.padre_id = NEW.id THEN
        RAISE EXCEPTION 'una ubicación no puede ser su propio padre';
    END IF;

    SELECT tipo INTO tipo_padre FROM ubicaciones WHERE id = NEW.padre_id;
    IF tipo_padre IS NULL THEN
        RAISE EXCEPTION 'la ubicación padre no existe';
    END IF;

    IF NOT (
        (tipo_padre = 'EDIFICIO' AND NEW.tipo = 'AREA') OR
        (tipo_padre = 'AREA' AND NEW.tipo = 'ESTANTE') OR
        (tipo_padre = 'ESTANTE' AND NEW.tipo = 'CAJA')
    ) THEN
        RAISE EXCEPTION 'jerarquía inválida: % no puede colgar de %', NEW.tipo, tipo_padre;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ubicaciones_jerarquia
    BEFORE INSERT OR UPDATE ON ubicaciones
    FOR EACH ROW EXECUTE FUNCTION validar_jerarquia_ubicacion();

-- ---------------------------------------------------------------------------
-- Expedientes: ubicación y unicidad por unidad/año
-- ---------------------------------------------------------------------------
ALTER TABLE expedientes
    ADD COLUMN unidad text,
    ADD COLUMN anio smallint CHECK (anio BETWEEN 1900 AND 2200),
    ADD COLUMN ubicacion_id uuid REFERENCES ubicaciones(id) ON DELETE RESTRICT,
    ADD COLUMN actualizado_en timestamptz NOT NULL DEFAULT now();

ALTER TABLE expedientes DROP CONSTRAINT expedientes_numero_key;

-- La unicidad se parametriza por unidad y año; NULL se trata como un valor propio.
CREATE UNIQUE INDEX expedientes_numero_unico
    ON expedientes (COALESCE(unidad, ''), COALESCE(anio, 0), numero);

CREATE INDEX expedientes_por_ubicacion ON expedientes (ubicacion_id);
CREATE INDEX expedientes_por_estado ON expedientes (estado, numero);

-- ---------------------------------------------------------------------------
-- Reservas con vencimiento
-- ---------------------------------------------------------------------------
CREATE TABLE reservas (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    expediente_id uuid NOT NULL REFERENCES expedientes(id),
    pedido_item_id uuid NOT NULL REFERENCES pedido_items(id) ON DELETE CASCADE,
    reservada_por uuid NOT NULL REFERENCES usuarios(id),
    creada_en timestamptz NOT NULL DEFAULT now(),
    expira_en timestamptz NOT NULL,
    liberada_en timestamptz,
    motivo_liberacion text,
    CHECK (expira_en > creada_en)
);

CREATE UNIQUE INDEX reservas_activas ON reservas (expediente_id) WHERE liberada_en IS NULL;
CREATE INDEX reservas_por_item ON reservas (pedido_item_id);
CREATE INDEX reservas_vencidas ON reservas (expira_en) WHERE liberada_en IS NULL;

-- ---------------------------------------------------------------------------
-- Descartes con motivo catalogado
-- ---------------------------------------------------------------------------
CREATE TABLE motivos_descarte (
    clave text PRIMARY KEY
        CHECK (clave IN ('NO_LOCALIZADO', 'NO_DISPONIBLE', 'EN_CUSTODIA_OTRO', 'REGISTRO_ERRONEO', 'OTRO')),
    descripcion text NOT NULL,
    requiere_detalle boolean NOT NULL DEFAULT false
);

CREATE TABLE descartes (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    pedido_item_id uuid NOT NULL UNIQUE REFERENCES pedido_items(id) ON DELETE CASCADE,
    motivo_clave text NOT NULL REFERENCES motivos_descarte(clave),
    detalle text,
    usuario_id uuid NOT NULL REFERENCES usuarios(id),
    creado_en timestamptz NOT NULL DEFAULT now(),
    CHECK (motivo_clave <> 'OTRO' OR (detalle IS NOT NULL AND length(trim(detalle)) >= 5))
);

-- ---------------------------------------------------------------------------
-- Acuses de recepción e incidencias
-- ---------------------------------------------------------------------------
CREATE TABLE acuses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    entrega_id uuid NOT NULL UNIQUE REFERENCES entregas(id),
    receptor_id uuid NOT NULL REFERENCES usuarios(id),
    sesion_id uuid REFERENCES sesiones(id) ON DELETE SET NULL,
    dispositivo_id uuid REFERENCES dispositivos(id) ON DELETE SET NULL,
    firmado_en timestamptz NOT NULL DEFAULT now(),
    ip inet,
    observaciones text
);

CREATE TABLE incidencias (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    expediente_id uuid NOT NULL REFERENCES expedientes(id),
    entrega_id uuid REFERENCES entregas(id),
    tipo text NOT NULL,
    descripcion text NOT NULL,
    estado text NOT NULL DEFAULT 'ABIERTA'
        CHECK (estado IN ('ABIERTA', 'EN_REVISION', 'RESUELTA', 'DESCARTADA')),
    reportada_por uuid NOT NULL REFERENCES usuarios(id),
    creada_en timestamptz NOT NULL DEFAULT now(),
    resuelta_en timestamptz
);

CREATE INDEX incidencias_abiertas ON incidencias (estado, creada_en DESC);

-- ---------------------------------------------------------------------------
-- Auditoría append-only
-- ---------------------------------------------------------------------------
CREATE TABLE auditoria (
    id bigserial PRIMARY KEY,
    ocurrido_en timestamptz NOT NULL DEFAULT now(),
    actor_id uuid REFERENCES usuarios(id),
    sesion_id uuid,
    dispositivo_id uuid,
    accion text NOT NULL,
    entidad text NOT NULL,
    entidad_id uuid,
    pedido_id uuid,
    expediente_id uuid,
    resultado text NOT NULL CHECK (resultado IN ('EXITO', 'RECHAZO', 'ERROR')),
    detalle jsonb NOT NULL DEFAULT '{}'::jsonb,
    correlacion_id uuid NOT NULL DEFAULT gen_random_uuid(),
    ip inet
);

CREATE INDEX auditoria_por_expediente ON auditoria (expediente_id, ocurrido_en DESC);
CREATE INDEX auditoria_por_actor ON auditoria (actor_id, ocurrido_en DESC);
CREATE INDEX auditoria_por_correlacion ON auditoria (correlacion_id);

-- Se ignoran silenciosamente las modificaciones y borrados, incluso para el propietario.
CREATE RULE auditoria_no_update AS ON UPDATE TO auditoria DO INSTEAD NOTHING;
CREATE RULE auditoria_no_delete AS ON DELETE TO auditoria DO INSTEAD NOTHING;

-- ---------------------------------------------------------------------------
-- Sincronización
-- ---------------------------------------------------------------------------
CREATE TABLE sync_events (
    secuencia bigserial PRIMARY KEY,
    entidad text NOT NULL,
    entidad_id uuid NOT NULL,
    operacion text NOT NULL CHECK (operacion IN ('CREAR', 'ACTUALIZAR', 'ELIMINAR')),
    version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
    tombstone boolean NOT NULL DEFAULT false,
    visibilidad text NOT NULL DEFAULT 'GLOBAL'
        CHECK (visibilidad IN ('GLOBAL', 'ROL', 'USUARIO')),
    rol_clave text,
    usuario_id uuid REFERENCES usuarios(id) ON DELETE SET NULL,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    ocurrido_en timestamptz NOT NULL DEFAULT now(),
    dispositivo_origen uuid REFERENCES dispositivos(id) ON DELETE SET NULL,
    CHECK (visibilidad <> 'ROL' OR rol_clave IS NOT NULL),
    CHECK (visibilidad <> 'USUARIO' OR usuario_id IS NOT NULL)
);

CREATE INDEX sync_events_entidad ON sync_events (entidad, entidad_id, secuencia DESC);
CREATE INDEX sync_events_por_rol ON sync_events (rol_clave, secuencia) WHERE visibilidad = 'ROL';

CREATE TABLE sync_devices (
    dispositivo_id uuid PRIMARY KEY REFERENCES dispositivos(id) ON DELETE CASCADE,
    ultima_secuencia bigint NOT NULL DEFAULT 0 CHECK (ultima_secuencia >= 0),
    ultima_sync timestamptz
);

CREATE TABLE operaciones_idempotentes (
    clave uuid NOT NULL,
    dispositivo_id uuid NOT NULL REFERENCES dispositivos(id) ON DELETE CASCADE,
    tipo text NOT NULL,
    payload_hash text NOT NULL,
    estado text NOT NULL DEFAULT 'EN_PROCESO'
        CHECK (estado IN ('EN_PROCESO', 'COMPLETADA', 'RECHAZADA')),
    respuesta jsonb,
    creada_en timestamptz NOT NULL DEFAULT now(),
    completada_en timestamptz,
    PRIMARY KEY (clave, dispositivo_id),
    CHECK (estado <> 'COMPLETADA' OR respuesta IS NOT NULL)
);

-- ---------------------------------------------------------------------------
-- Semillas de catálogo (sin secretos)
-- ---------------------------------------------------------------------------
INSERT INTO roles (clave, nombre, descripcion) VALUES
    ('ADMINISTRADOR', 'Administrador', 'Configuración, usuarios y roles'),
    ('ALMACEN', 'Almacén', 'Preparación, descarte y entrega de expedientes'),
    ('USUARIO', 'Usuario solicitante', 'Solicita expedientes y confirma recepción'),
    ('CONSULTA', 'Consulta', 'Solo lectura de información autorizada');

INSERT INTO permisos (clave, descripcion) VALUES
    ('pedidos:crear', 'Crear pedidos de expedientes'),
    ('pedidos:leer', 'Consultar pedidos propios o autorizados'),
    ('pedidos:preparar', 'Preparar renglones de un pedido'),
    ('pedidos:descartar', 'Descartar renglones con motivo'),
    ('entregas:despachar', 'Despachar entregas preparadas'),
    ('entregas:confirmar', 'Confirmar recepción de una entrega'),
    ('devoluciones:solicitar', 'Solicitar devolución de un expediente'),
    ('devoluciones:confirmar', 'Confirmar recepción de una devolución en almacén'),
    ('transferencias:aceptar', 'Aceptar una transferencia de custodia'),
    ('expedientes:leer', 'Consultar expedientes autorizados'),
    ('expedientes:administrar', 'Alta y modificación de expedientes'),
    ('reportes:leer', 'Consultar reportes'),
    ('admin:usuarios', 'Administrar usuarios'),
    ('admin:roles', 'Administrar roles y permisos'),
    ('auditoria:leer', 'Consultar la bitácora de auditoría'),
    ('sync:push', 'Enviar operaciones locales al servidor'),
    ('sync:pull', 'Recibir eventos de sincronización');

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id FROM roles r CROSS JOIN permisos p WHERE r.clave = 'ADMINISTRADOR';

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id FROM roles r JOIN permisos p ON p.clave IN (
    'pedidos:leer', 'pedidos:preparar', 'pedidos:descartar',
    'entregas:despachar', 'devoluciones:confirmar',
    'expedientes:leer', 'expedientes:administrar', 'reportes:leer',
    'auditoria:leer', 'sync:push', 'sync:pull'
) WHERE r.clave = 'ALMACEN';

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id FROM roles r JOIN permisos p ON p.clave IN (
    'pedidos:crear', 'pedidos:leer', 'entregas:confirmar',
    'devoluciones:solicitar', 'transferencias:aceptar',
    'expedientes:leer', 'sync:push', 'sync:pull'
) WHERE r.clave = 'USUARIO';

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id FROM roles r JOIN permisos p ON p.clave IN (
    'pedidos:leer', 'expedientes:leer', 'reportes:leer'
) WHERE r.clave = 'CONSULTA';

INSERT INTO motivos_descarte (clave, descripcion, requiere_detalle) VALUES
    ('NO_LOCALIZADO', 'No localizado en su ubicación registrada', false),
    ('NO_DISPONIBLE', 'No disponible al momento de la preparación', false),
    ('EN_CUSTODIA_OTRO', 'En custodia de otro usuario', false),
    ('REGISTRO_ERRONEO', 'Registro erróneo del pedido', false),
    ('OTRO', 'Otro motivo, requiere descripción', true);

INSERT INTO ubicaciones (tipo, codigo, nombre) VALUES
    ('EDIFICIO', 'ED-01', 'Edificio principal');

INSERT INTO ubicaciones (padre_id, tipo, codigo, nombre)
SELECT id, 'AREA', 'ARCHIVO', 'Archivo judicial'
FROM ubicaciones WHERE tipo = 'EDIFICIO' AND codigo = 'ED-01';
