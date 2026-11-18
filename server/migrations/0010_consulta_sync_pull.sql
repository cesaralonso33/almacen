-- El rol CONSULTA puede registrar su dispositivo y descargar eventos.
-- No recibe sync:push: sigue siendo un rol de solo lectura.
INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave = 'sync:pull'
WHERE r.clave = 'CONSULTA'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;
