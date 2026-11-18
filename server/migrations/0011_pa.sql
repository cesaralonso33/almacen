-- Módulo P.A.: registro de personas con folio consecutivo por año.
-- Los datos personales son sensibles: el acceso se controla por permisos y
-- todas las mutaciones quedan auditadas.

CREATE TABLE pa_folios (
    anio smallint PRIMARY KEY,
    ultimo integer NOT NULL CHECK (ultimo > 0)
);

CREATE TABLE pa_registros (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    fecha date NOT NULL,
    anio smallint NOT NULL,
    folio text NOT NULL UNIQUE,
    curp text NOT NULL,
    nombre text NOT NULL,
    genero text NOT NULL,
    municipio text NOT NULL,
    edad smallint NOT NULL CHECK (edad >= 0 AND edad <= 150),
    observaciones text NOT NULL DEFAULT '',
    campos_extra jsonb NOT NULL DEFAULT '[]'::jsonb,
    documento bytea NOT NULL,
    documento_nombre text NOT NULL,
    creado_por uuid REFERENCES usuarios(id),
    actualizado_por uuid REFERENCES usuarios(id),
    creado_en timestamptz NOT NULL DEFAULT now(),
    actualizado_en timestamptz NOT NULL DEFAULT now(),
    eliminado_en timestamptz
);

CREATE INDEX pa_por_fecha ON pa_registros (fecha DESC) WHERE eliminado_en IS NULL;
CREATE INDEX pa_por_curp ON pa_registros (curp) WHERE eliminado_en IS NULL;
CREATE INDEX pa_por_nombre ON pa_registros (lower(nombre)) WHERE eliminado_en IS NULL;
CREATE INDEX pa_por_municipio ON pa_registros (lower(municipio)) WHERE eliminado_en IS NULL;
CREATE INDEX pa_nombre_trgm ON pa_registros USING gin (nombre gin_trgm_ops)
    WHERE eliminado_en IS NULL;
CREATE INDEX pa_curp_trgm ON pa_registros USING gin (curp gin_trgm_ops)
    WHERE eliminado_en IS NULL;
CREATE INDEX pa_municipio_trgm ON pa_registros USING gin (municipio gin_trgm_ops)
    WHERE eliminado_en IS NULL;

INSERT INTO permisos (clave, descripcion) VALUES
    ('pa:leer', 'Consultar registros P.A.'),
    ('pa:crear', 'Crear registros P.A.'),
    ('pa:editar', 'Editar registros P.A.'),
    ('pa:eliminar', 'Eliminar lógicamente registros P.A.')
ON CONFLICT (clave) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave IN ('pa:leer', 'pa:crear', 'pa:editar', 'pa:eliminar')
WHERE r.clave = 'ADMINISTRADOR'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave IN ('pa:leer', 'pa:crear', 'pa:editar')
WHERE r.clave IN ('ALMACEN', 'USUARIO')
ON CONFLICT (rol_id, permiso_id) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave = 'pa:leer'
WHERE r.clave = 'CONSULTA'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;
