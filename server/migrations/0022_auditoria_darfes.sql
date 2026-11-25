-- Auditoría detallada del módulo DARFES.
-- El historial se guarda en la bitácora append-only existente; este permiso
-- controla únicamente su consulta y se reserva al rol Administrador.

INSERT INTO permisos (clave, descripcion)
VALUES ('darfes:auditoria', 'Consultar el historial de cambios de los DARFES')
ON CONFLICT (clave) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r, permisos p
WHERE r.clave = 'ADMINISTRADOR' AND p.clave = 'darfes:auditoria'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;

DELETE FROM rol_permisos rp
USING roles r, permisos p
WHERE rp.rol_id = r.id
  AND rp.permiso_id = p.id
  AND p.clave = 'darfes:auditoria'
  AND r.clave <> 'ADMINISTRADOR';

CREATE INDEX IF NOT EXISTS auditoria_entidad_fecha_idx
    ON auditoria (entidad, entidad_id, ocurrido_en DESC);
