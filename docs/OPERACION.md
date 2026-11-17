# Operación y contingencia

## Rutina diaria

| Actividad | Comando / lugar |
|---|---|
| Estado de servicios | `docker compose ps` |
| Salud de la API | `set -a; . ./.env; set +a; curl -fsS "https://$SITIO/health"` |
| Logs de la API | `docker compose logs -f api` |
| Logs del proxy | `infra/caddy_data` → `access.log` (rotación 10 MiB, 10 archivos) |
| Pedidos y entregas | Panel «Inicio» de la aplicación |
| Conflictos de sincronización | Botón «conflictos» en la barra superior |

## Respaldos

```bash
cd /opt/expedientes/infra
chmod +x backup/backup.sh restore/restore.sh
./backup/backup.sh
```
- Salida: `infra/backup/salidas/expedientes_<UTC>.dump` (formato custom de PostgreSQL).
- Retención: `RETENCION_DIAS` (30 por omisión).
- Programe con cron, por ejemplo a las 02:00 y 14:00:

```cron
0 2,14 * * * cd /opt/expedientes/infra && bash ./backup/backup.sh >> backup/registro.log 2>&1
```

Copie los respaldos fuera del servidor (almacenamiento institucional). Un respaldo que solo vive
en el mismo disco no protege contra fallo de hardware.

## Limpieza de inventario (mantenimiento)

Para empezar de cero antes de cargar el inventario real:

```bash
cd /opt/expedientes/infra
chmod +x backup/backup.sh mantenimiento/limpiar-expedientes.sh
./mantenimiento/limpiar-expedientes.sh
```

Pide escribir `BORRAR`, crea un respaldo previo y luego vacía:

- expedientes, pedidos, renglones, reservas, entregas, acuses, custodias
- movimientos, descartes, devoluciones, transferencias e incidencias
- eventos de sincronización y claves de idempotencia (para que los clientes
  reciban un snapshot vacío)

**Conserva** usuarios, roles, permisos, ubicaciones y la bitácora de auditoría.

Después de la limpieza, cada cliente debe pulsar **Sincronizar**; al detectar un
cursor inválido pedirá el snapshot y borrará su caché local de expedientes.

## Restauración (probada)

```bash
cd /opt/expedientes/infra
./restore/restore.sh backup/salidas/expedientes_20260101T030000Z.dump
```

El script pide confirmación, crea un respaldo previo, detiene la API, recrea la base y restaura.
Después verifique `/health`, el inicio de sesión y la bitácora.

## Contingencia sin servidor

1. La aplicación muestra `Sin conexión` y cuenta los pendientes.
2. Se pueden consultar expedientes ya sincronizados y crear pedidos que quedan en el outbox.
3. **No** se puede confirmar custodia, entrega, devolución ni permiso sin servidor.
4. Al volver la conexión, pulse «Sincronizar»; el push es idempotente y no duplica pedidos.
5. Si el servidor rechaza una operación, aparecerá como conflicto; requiere revisión humana.

## Incidentes

| Síntoma | Revisión |
|---|---|
| 401 generalizado | Reloj del servidor, expiración de sesión, `JWT_SECRET` cambiado |
| 403 en un rol | Permisos del rol en `/admin/roles` |
| 429 al iniciar sesión | Bloqueo por intentos fallidos (15 min) |
| 500 al preparar | Revisar logs de la API y restricciones de custodia/reserva |
| La API no arranca | Migración fallida; restaurar respaldo y revisar `docker compose logs api` |

## Rotación de secretos

- `POSTGRES_PASSWORD`: actualizar `.env`, recrear el contenedor de PostgreSQL y la API.
- `JWT_SECRET`: invalida todos los accesos; los usuarios deben volver a iniciar sesión (el refresh
  también deja de servir). Programar en ventana de mantenimiento.

## Capacidad

El sistema está dimensionado para ~20 usuarios y decenas de miles de expedientes. Los índices
cubren búsqueda por número, pedidos por solicitante, movimientos y eventos de sincronización.
Si el volumen crece, revisar `pg_stat_statements` antes de añadir índices.
