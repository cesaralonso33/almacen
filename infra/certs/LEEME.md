# Certificados TLS institucionales

Coloque aquí `tls.crt` (cadena completa) y `tls.key` (llave privada, permisos 600).
Los archivos están ignorados por Git.

Si usa la CA interna de Caddy (`Caddyfile.interno`), no necesita archivos; extraiga la raíz
para instalarla en los clientes:

```bash
docker compose exec caddy cat /data/caddy/pki/authorities/local/root.crt > raiz-institucional.crt
```
