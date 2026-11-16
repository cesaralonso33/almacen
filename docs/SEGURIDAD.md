# Seguridad

## Autenticación

- Contraseñas con **Argon2id** (sal aleatoria); el hash nunca se expone.
- **JWT HS256** de acceso de 15 minutos con `sub` y `sid`; cada petición valida que la sesión
  siga viva, no revocada y que el usuario esté activo.
- **Refresh opaco** de 32 bytes, almacenado como SHA-256, rotativo y de un solo uso.
- Cambio de contraseña y desactivación revocan todas las sesiones.
- Límite de 5 intentos fallidos por correo en 15 minutos (429).

## Autorización

- RBAC con permisos atómicos (`pedidos:preparar`, `entregas:confirmar`, `admin:usuarios`, …).
- La identidad **solo** proviene del token; `actor_id`, `solicitante_id` y `receptor_id` están
  prohibidos en los cuerpos (`deny_unknown_fields` → 400).
- El receptor de una entrega y el custodio de un expediente se comparan contra la sesión.
- Un usuario sin `pedidos:preparar` no ve pedidos ajenos (404, sin filtrar existencia).

## Transporte y secretos

- TLS terminado en Caddy con la CA institucional; HSTS, `X-Content-Type-Options`,
  `X-Frame-Options`, `Referrer-Policy` y `Permissions-Policy`.
- Solo Caddy publica puertos; PostgreSQL y la API viven en la red interna de contenedores.
- `JWT_SECRET` y `POSTGRES_PASSWORD` vienen del entorno; `.env` está ignorado por Git.
- La aplicación de escritorio guarda el token en memoria del proceso; nunca en la base local.
- Al instalar, la CA institucional debe estar en el almacén de certificados del cliente.

## Auditoría

- Tabla `auditoria` **append-only** (reglas `DO INSTEAD NOTHING` para UPDATE y DELETE).
- Registra actor, sesión, acción, entidad, resultado, detalle JSON y correlación.
- Las operaciones de negocio escriben su evento en la misma transacción.

## Riesgos abiertos

| Riesgo | Mitigación recomendada |
|---|---|
| SQLite local sin cifrar | Cifrado de disco (BitLocker/LUKS) en los equipos |
| Sin segundo factor | Evaluar 2FA institucional |
| Límite de login por correo | Complementar con límite por IP en el proxy |
| Usuario de base con privilegios de propietario | Crear un rol de aplicación con permisos mínimos |
| Sin pinning de certificado en el cliente | Validación por CA institucional instalada |
