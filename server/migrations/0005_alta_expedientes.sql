-- 0005: los usuarios registran expedientes y quedan como custodios iniciales.

INSERT INTO permisos (clave, descripcion)
VALUES ('expedientes:crear', 'Registrar expedientes y quedar como custodio inicial')
ON CONFLICT (clave) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave = 'expedientes:crear'
WHERE r.clave IN ('ADMINISTRADOR', 'ALMACEN', 'USUARIO')
ON CONFLICT DO NOTHING;
