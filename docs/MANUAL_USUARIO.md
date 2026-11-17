# Manual rápido de uso — Custodia CradSystem

Guía breve para operar el sistema. Cada sección explica solo lo necesario.

## Antes de empezar

- La aplicación se instala en cada equipo; la dirección del servidor viene precargada.
- El administrador crea los usuarios y entrega una contraseña temporal.
- En el primer acceso, cada persona debe cambiarla con el botón **Contraseña** del menú lateral.

| Rol | Puede hacer |
|---|---|
| **ADMINISTRADOR** | Todo: usuarios, importación, reportes, operación |
| **ALMACEN** | Preparar, descartar, despachar, acusar devoluciones, importar |
| **USUARIO** | Registrar expedientes, pedir, recibir, devolver, transferir |
| **CONSULTA** | Solo lectura y reportes |

## 1. Entrar

1. Escribe tu correo y contraseña.
2. Pulsa **Iniciar sesión**.
3. Arriba a la derecha verás el estado: **En línea**, **Sin conexión** o **Error**.

La sesión dura **18 horas**. Si pasan **30 minutos sin usar** teclado o ratón, se cierra sola y vuelve al acceso.

## 2. Registrar un expediente

Dos caminos, el mismo resultado:

- **Expedientes → Nuevo expediente**
- **Mis custodias → Agregar expediente**

Llena número, unidad (opcional), año y descripción.

- El número se completa a **8 caracteres antes de la barra**: `5259/2026` se guarda como `00005259/2026`.
- El **año** ya viene con el actual.
- El número es **único en todo el sistema**: si se repite, aparece un aviso.
- Al guardarlo queda **bajo tu custodia** hasta que lo devuelvas al almacén.

El administrador o almacén también puede cargar muchos de golpe:
**Expedientes → Importar lista**, con una línea por expediente: `numero;descripcion`.

## 3. Pedir expedientes

1. **Mis pedidos → Nuevo pedido**.
2. Busca por número o descripción.
3. Marca uno o varios expedientes.
4. Pulsa **Crear pedido**.

El pedido queda **PENDIENTE** y aparece en la bandeja del almacén.

## 4. Preparar y entregar (rol Almacén)

1. Entra a **Almacén** y abre un pedido.
2. Por cada renglón:
   - **Preparar**: aparta el expediente.
   - **Descartar**: indica el motivo (no localizado, no disponible, en custodia de otro, registro erróneo u otro).
3. Pulsa **Despachar preparados**.

Después de despachar, el pedido queda **EN_PROCESO**; el receptor debe confirmar.

## 5. Confirmar recepción (usuario)

1. Entra a **Entregas**.
2. Verás las entregas pendientes de tu acuse.
3. Pulsa **Confirmar recepción**.

Solo en ese momento el expediente pasa a **Mis custodias**. Antes de confirmar, la custodia sigue con quien lo tenía.

## 6. Devolver al almacén

1. **Mis custodias → Devolver**.
2. El almacén entra a **Devoluciones** y pulsa **Acusar recepción**.

La custodia se cierra **solo** con ese acuse. Mientras tanto, el expediente sigue a tu cargo.

## 7. Transferir a otro usuario

1. **Mis custodias → Transferir**.
2. Elige el usuario destino y pulsa **Solicitar transferencia**.
3. El receptor entra a **Mis custodias → Transferencias por aceptar** y pulsa **Aceptar** o **Rechazar**.

Hasta que acepte, el custodio sigues siendo tú.

## 8. Buscar, filtrar y ver totales

En **Expedientes**:

- Búsqueda rápida por número o descripción.
- **Total** de expedientes arriba.
- Chips por estado: Almacén, En custodia, No localizado (clic para filtrar).
- **Filtros avanzados**: estado, unidad, año y custodio.
- Paginación al pie de la tabla.

## 9. Reportes

Disponible para administración, almacén y consulta:

- Contadores generales.
- **Expedientes nuevos por usuario**, con selector **Día / Mes / Año**.
- Pedidos por estado y detalle.

## 10. Administración

1. **Administración → Nuevo usuario**: nombre, correo, contraseña inicial y rol.
2. **Editar**: cambia nombre, correo o contraseña. Si cambias la contraseña, el usuario deberá entrar de nuevo.
3. **Activar / Desactivar**: al desactivar se cierran sus sesiones.
4. **Importar lista**: carga masiva de expedientes al almacén.

## 11. Sin conexión

- Puedes consultar los expedientes ya sincronizados (verás un aviso de copia local).
- Puedes crear pedidos: quedan en cola como **pendientes de sincronizar**.
- **No** puedes confirmar custodia, entregas, devoluciones ni transferencias.
- Al volver la red, pulsa **Sincronizar**; los pedidos se envían una sola vez.

## 12. Reglas que no se pueden saltar

- Un expediente tiene **una sola custodia activa**.
- La custodia **solo cambia con un acuse** autenticado.
- Un **descarte** no borra el expediente ni su historial.
- No se puede **cancelar** un pedido con renglones entregados o en tránsito.
- El número de expediente es **único**.
- Una devolución y una transferencia no pueden estar activas a la vez para el mismo expediente.

## Problemas comunes

| Mensaje | Qué hacer |
|---|---|
| «Sin conexión» | Revisa la red o la dirección del servidor; prueba con **Sincronizar** |
| «Credenciales inválidas» | Verifica correo y contraseña; tras 5 fallos espera 15 minutos |
| «Sesión cerrada por inactividad» | Vuelve a iniciar sesión |
| «Ya existe un expediente con ese número» | Busca el número; el sistema lo normaliza a 8 caracteres antes de `/` |
| «No tiene permiso» (403) | Tu rol no incluye esa acción; pide al administrador |
| «El expediente ya está reservado» | Otro pedido lo tiene preparado; espera o coordina con almacén |
| No aparece un botón | Depende de tu rol; los paneles se muestran según permisos |

## Glosario

- **Pedido**: solicitud de uno o varios expedientes.
- **Renglón**: cada expediente dentro de un pedido.
- **Preparar**: apartar el expediente para entregarlo.
- **Despachar**: enviarlo físicamente; la custodia aún no cambia.
- **Acuse**: confirmación autenticada de recepción.
- **Custodia**: responsabilidad oficial sobre el expediente.
- **Descarte**: no atender un renglón, con motivo.
