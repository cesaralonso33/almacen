-- Configuración de integraciones externas (Paperless-ngx y consulta de expedientes).
CREATE TABLE integraciones (
    clave text PRIMARY KEY,
    url text NOT NULL DEFAULT '',
    token text NOT NULL DEFAULT '',
    activo boolean NOT NULL DEFAULT false,
    config jsonb NOT NULL DEFAULT '{}'::jsonb,
    actualizado_en timestamptz NOT NULL DEFAULT now(),
    actualizado_por uuid REFERENCES usuarios(id)
);

INSERT INTO integraciones (clave) VALUES ('EXPEDIENTE'), ('PAPERLESS')
ON CONFLICT (clave) DO NOTHING;

INSERT INTO permisos (clave, descripcion) VALUES
    ('admin:integraciones', 'Configurar integraciones externas')
ON CONFLICT (clave) DO NOTHING;

DELETE FROM rol_permisos
USING roles r, permisos p
WHERE rol_permisos.rol_id = r.id
  AND rol_permisos.permiso_id = p.id
  AND p.clave = 'admin:integraciones'
  AND r.clave <> 'ADMINISTRADOR';

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave = 'admin:integraciones'
WHERE r.clave = 'ADMINISTRADOR'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;
