# Seguridad

## Autenticación

- Contraseñas con **Argon2id** (sal aleatoria); el hash nunca se expone.
- **JWT HS256** de acceso de 18 horas con `sub` y `sid`; cada petición valida que la sesión
  siga viva, no revocada y que el usuario esté activo.
- **Refresh opaco** de 32 bytes, almacenado como SHA-256, rotativo, de un solo uso y con 24 horas
  de vigencia; el escritorio lo usa para renovar el acceso sin volver a pedir la contraseña.
- Cambio de contraseña y desactivación revocan todas las sesiones.
- Límite de 5 intentos fallidos por correo en 15 minutos (429). El limitador es en memoria;
  complemente con límite por IP en el proxy y fail2ban.
- Cuerpo máximo de 2 MiB, timeout de 60 segundos y captura de pánicos para evitar caídas.
- Cabeceras de seguridad en todas las respuestas: `nosniff`, `DENY`, `no-referrer`,
  `no-store`, CSP restrictiva, HSTS y `Permissions-Policy`.
- Correo máximo de 254 caracteres y contraseña máxima de 512 para evitar entradas abusivas.
- La sesión dura como máximo **24 horas** y se cierra tras **18 horas sin actividad del usuario**
  (`IDLE_TTL_MINUTOS`). La sincronización automática no cuenta como actividad.

## Autorización

- RBAC con permisos atómicos (`pedidos:preparar`, `entregas:confirmar`, `admin:usuarios`, …).
- La identidad **solo** proviene del token; `actor_id`, `solicitante_id` y `receptor_id` están
  prohibidos en los cuerpos (`deny_unknown_fields` → 400).
- El receptor de una entrega y el custodio de un expediente se comparan contra la sesión.
- Un usuario sin `pedidos:preparar` no ve pedidos ajenos (404, sin filtrar existencia).
- El rol `CONSULTA` puede registrar su dispositivo y descargar eventos (`sync:pull`), pero no
  enviar operaciones (`sync:push`).
- P.A. contiene datos personales sensibles. `pa:leer` se asigna a todos los roles; crear/editar a
  Administración, Almacén y Usuario; eliminar solo a Administración. Cada mutación y cada consulta
  de documento requieren sesión y quedan sujetas a auditoría.
- DARFES está reservado al perfil `ADMINISTRATIVO` y a `ADMINISTRADOR`. Los documentos adjuntos se
  limitan a 8 MiB, se guardan en la base y requieren `darfes:documentos`; la exportación y los
  reportes tienen permisos separados.
- `ADMINISTRADOR` omite la comprobación de permisos en `AuthUser::puede`, de modo que conserva
  acceso a todos los módulos presentes y futuros sin depender de que cada migración le conceda
  permisos. `ADMINISTRATIVO` sigue limitado a sus permisos explícitos.
- El PDF de depósito se procesa en el servidor: extrae número de cuenta, monto y nombre, compara
  contra lo capturado y guarda `coincide` y las diferencias. La falta de coincidencia no descarta
  la carga; queda visible para revisión humana y en la auditoría.
- En los reportes SPEI se toma la cuenta/CLABE y el nombre del **beneficiario**; la cuenta y el
  nombre del **ordenante** se ignoran para no comparar contra los datos de la institución.
- La generación del recibo PDF exige `darfes:editar`, asigna el folio dentro de una transacción con
  bloqueo de la fila del DARFE y registra `darfes.formato` en la auditoría.
- `darfes:folio` y `darfes:duplicar` se conceden únicamente a `ADMINISTRADOR`. El primero habilita
  la edición del folio del recibo; el segundo, el botón **Duplicar** del formulario.
- El recibo se entrega como DOCX generado a partir de `server/assets/plantillas/recibo_ayuda.docx`.
  La plantilla no contiene datos personales; solo marcadores que el servidor sustituye.

## Transporte y secretos

- TLS terminado en Caddy con la CA institucional; HSTS, `X-Content-Type-Options`,
  `X-Frame-Options`, `Referrer-Policy` y `Permissions-Policy`.
- Solo Caddy publica puertos; PostgreSQL y la API viven en la red interna de contenedores.
- `JWT_SECRET` y `POSTGRES_PASSWORD` vienen del entorno; `.env` está ignorado por Git.
- La aplicación de escritorio guarda los tokens en memoria del proceso; nunca en la base local.
- La SQLite local se cifra con SQLCipher. La clave se guarda en el Administrador de credenciales
  de Windows y queda protegida por DPAPI; una base antigua se migra sin perder el outbox.
- El updater verifica la firma minisign del instalador; la llave privada no se versiona.
- Al instalar, la CA institucional debe estar en el almacén de certificados del cliente.

## Auditoría

- Tabla `auditoria` **append-only** (reglas `DO INSTEAD NOTHING` para UPDATE y DELETE).
- Registra actor, sesión, acción, entidad, resultado, detalle JSON y correlación.
- Las operaciones de negocio escriben su evento en la misma transacción.

## Riesgos abiertos

| Riesgo | Mitigación recomendada |
|---|---|
| Pérdida de la credencial local de Windows | Restaurar desde el servidor/snapshot; respaldar el perfil y la llave del updater |
| Sin segundo factor | Evaluar 2FA institucional |
| Límite de login por correo | Complementar con límite por IP en el proxy |
| Usuario de base con privilegios de propietario | Crear un rol de aplicación con permisos mínimos |
| Sin pinning de certificado en el cliente | Validación por CA institucional instalada |
| Modo HTTP sin TLS (`Caddyfile.http`) | Solo con firewall/VPN; migrar a TLS cuanto antes |
| Instalador sin Authenticode | Firmar con el certificado de código institucional cuando esté disponible |
