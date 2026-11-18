-- Permite al administrador crear pedidos a nombre de otro usuario.
-- No cambia la custodia: el pedido sigue el flujo de almacén y acuse.
INSERT INTO permisos (clave, descripcion)
VALUES ('pedidos:asignar', 'Crear pedidos a nombre de otro usuario')
ON CONFLICT (clave) DO NOTHING;

INSERT INTO rol_permisos (rol_id, permiso_id)
SELECT r.id, p.id
FROM roles r
JOIN permisos p ON p.clave = 'pedidos:asignar'
WHERE r.clave = 'ADMINISTRADOR'
ON CONFLICT (rol_id, permiso_id) DO NOTHING;
