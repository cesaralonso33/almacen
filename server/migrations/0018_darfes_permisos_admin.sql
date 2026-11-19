-- Permisos exclusivos del Administrador para folios y duplicado de DARFES.
INSERT INTO permisos (clave, descripcion) VALUES
    ('darfes:folio', 'Editar el folio del recibo de un DARFE'),
    ('darfes:duplicar', 'Duplicar registros de DARFES')
ON CONFLICT (clave) DO NOTHING;

-- Garantiza que ningún otro rol conserve estos permisos.
DELETE FROM rol_permisos
USING roles r, permisos p
WHERE rol_permisos.rol_id = r.id
  AND rol_permisos.permiso_id = p.id
  AND p.clave IN ('darfes:folio', 'darfes:duplicar')
  AND r.clave <> 'ADMINISTRADOR';

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave IN ('darfes:folio', 'darfes:duplicar')
WHERE r.clave = 'ADMINISTRADOR'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;
