# Expedientes jurídicos — starter técnico

Este proyecto es un **prototipo de desarrollo**, no un sistema listo para producción.

## Requisitos
Rust estable, Node.js 20+, npm, Docker Compose, dependencias de Tauri 2 para tu sistema operativo.

## Iniciar PostgreSQL
```bash
cd infra
printf 'POSTGRES_PASSWORD=clave_de_desarrollo\n' > .env
docker compose up -d
```

## Iniciar API
```bash
cd server
export DATABASE_URL='postgres://expedientes:clave_de_desarrollo@localhost:5432/expedientes'
cargo run
```
La API ejecuta migraciones al iniciar y escucha en `http://localhost:3000`. `GET /health` comprueba disponibilidad.

## Iniciar escritorio
```bash
cd desktop
npm install
npm run tauri dev
```

## Endpoints implementados
- POST /api/v1/pedidos `{ "solicitante_id":"UUID", "expedientes":["UUID"] }`
- GET /api/v1/pedidos/{id}
- POST /api/v1/items/{id}/preparar `{ "actor_id":"UUID" }`
- POST /api/v1/items/{id}/descartar `{ "actor_id":"UUID", "motivo":"No localizado en estante" }`
- POST /api/v1/entregas `{ "actor_id":"UUID", "item_ids":["UUID"] }`
- POST /api/v1/entregas/{id}/confirmar `{ "receptor_id":"UUID" }`
- POST /api/v1/expedientes/{id}/devolver `{ "actor_id":"UUID" }`

Crear usuarios y expedientes de prueba directamente con SQL antes de usar los endpoints. Los IDs son UUID.

## Alcance y limitaciones críticas
- La API contiene transacciones para pedidos, descartes, despacho, confirmación y devolución. Permite entregas parciales seleccionando subconjuntos de items y completas seleccionando todos los preparados.
- El escritorio demuestra almacenamiento SQLite y outbox local; **no sincroniza todavía con el servidor**. La creación local es demostrativa y no se debe usar como registro oficial.
- No hay autenticación ni autorización de actor_id; cualquiera que alcance la API podría suplantar un usuario. Mantener aislada en desarrollo.
- No se implementan aún la firma del acuse, idempotencia, políticas de caducidad de reservas, eventos de sincronización, manejo de conflictos, control de ubicaciones, pruebas automatizadas ni TLS.
- Las devoluciones están simplificadas: el custodio registra directamente la devolución. En producción deben exigir recepción del almacén.
- El código está diseñado como base para extender y revisar; no ha sido compilado ni sometido a pruebas de integración en este entorno.
