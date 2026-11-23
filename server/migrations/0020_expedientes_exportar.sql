-- Exportación del inventario de expedientes, exclusiva del Administrador.
INSERT INTO permisos (clave, descripcion) VALUES
    ('expedientes:exportar', 'Exportar el inventario de expedientes')
ON CONFLICT (clave) DO NOTHING;

DELETE FROM rol_permisos
USING roles r, permisos p
WHERE rol_permisos.rol_id = r.id
  AND rol_permisos.permiso_id = p.id
  AND p.clave = 'expedientes:exportar'
  AND r.clave <> 'ADMINISTRADOR';

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave = 'expedientes:exportar'
WHERE r.clave = 'ADMINISTRADOR'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;
