-- Perfil Administrativo y módulo DARFES.
-- Los documentos se guardan como bytea para que respaldo/restauración sean atómicos.

INSERT INTO roles (clave, nombre, descripcion)
VALUES ('ADMINISTRATIVO', 'Administrativo', 'Gestión de DARFES y sus documentos')
ON CONFLICT (clave) DO NOTHING;

CREATE TABLE darfes (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    numero_darfe text NOT NULL UNIQUE,
    fecha date NOT NULL,
    nombre text NOT NULL,
    expediente text NOT NULL DEFAULT '',
    registro text NOT NULL DEFAULT '',
    estado_cuenta text NOT NULL DEFAULT '',
    banco text NOT NULL DEFAULT '',
    identificacion text NOT NULL DEFAULT '',
    curp text NOT NULL DEFAULT '',
    rfc text NOT NULL DEFAULT '',
    estatus text NOT NULL CHECK (estatus IN ('PENDIENTE', 'INCOMPLETO', 'COMPLETO')),
    campos_extra jsonb NOT NULL DEFAULT '[]'::jsonb,
    creado_por uuid REFERENCES usuarios(id),
    actualizado_por uuid REFERENCES usuarios(id),
    creado_en timestamptz NOT NULL DEFAULT now(),
    actualizado_en timestamptz NOT NULL DEFAULT now(),
    eliminado_en timestamptz
);

CREATE INDEX darfes_por_fecha ON darfes (fecha DESC) WHERE eliminado_en IS NULL;
CREATE INDEX darfes_por_estatus ON darfes (estatus, fecha DESC) WHERE eliminado_en IS NULL;
CREATE INDEX darfes_por_banco ON darfes (lower(banco)) WHERE eliminado_en IS NULL;
CREATE INDEX darfes_numero_trgm ON darfes USING gin (numero_darfe gin_trgm_ops)
    WHERE eliminado_en IS NULL;
CREATE INDEX darfes_nombre_trgm ON darfes USING gin (nombre gin_trgm_ops)
    WHERE eliminado_en IS NULL;

CREATE TABLE darfe_documentos (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    darfe_id uuid NOT NULL REFERENCES darfes(id) ON DELETE CASCADE,
    campo text NOT NULL CHECK (campo IN ('CURP', 'RFC', 'REGISTRO', 'IDENTIFICACION', 'ESTADO_CUENTA')),
    nombre text NOT NULL,
    mime text NOT NULL,
    tamano integer NOT NULL CHECK (tamano > 0),
    contenido bytea NOT NULL,
    creado_por uuid REFERENCES usuarios(id),
    creado_en timestamptz NOT NULL DEFAULT now(),
    UNIQUE (darfe_id, campo)
);

CREATE INDEX darfe_documentos_por_darfe ON darfe_documentos (darfe_id);

INSERT INTO permisos (clave, descripcion) VALUES
    ('darfes:leer', 'Consultar DARFES'),
    ('darfes:crear', 'Crear DARFES'),
    ('darfes:editar', 'Editar DARFES'),
    ('darfes:eliminar', 'Eliminar lógicamente DARFES'),
    ('darfes:documentos', 'Subir y eliminar documentos de DARFES'),
    ('darfes:exportar', 'Exportar DARFES a Excel'),
    ('darfes:reportes', 'Consultar reportes e indicadores de DARFES')
ON CONFLICT (clave) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave LIKE 'darfes:%'
WHERE r.clave IN ('ADMINISTRADOR', 'ADMINISTRATIVO')
ON CONFLICT (rol_id, permiso_id) DO NOTHING;

-- El perfil administrativo necesita iniciar sesión y descargar la caché autorizada.
INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave IN ('sync:pull', 'pedidos:leer')
WHERE r.clave = 'ADMINISTRATIVO'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;
