<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let custodias = $state<any[]>([]);
    let pendientes = $state<any[]>([]);
    let transferencia = $state<any>(null);
    let directorio = $state<{ id: string; nombre: string }[]>([]);
    let destino = $state('');
    let observaciones = $state('');
    let pagina = $state(1);

    let agregando = $state(false);
    let numero = $state('');
    let unidad = $state('');
    let anio = $state<number | null>(new Date().getFullYear());
    let descripcion = $state('');
    let guardando = $state(false);

    const paginas = $derived(totalPaginas(custodias.length, POR_PAGINA));
    const visibles = $derived(recortar(custodias, pagina, POR_PAGINA));

    onMount(() => {
        cargar();
        cargarPendientes();
    });

    async function cargar() {
        try {
            const respuesta = await api<{ custodias: any[] }>('GET', '/api/v1/custodias');
            custodias = respuesta.custodias;
            pagina = 1;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    function abrirAlta() {
        numero = '';
        unidad = '';
        descripcion = '';
        anio = new Date().getFullYear();
        agregando = true;
    }

    async function registrar() {
        guardando = true;
        try {
            await api('POST', '/api/v1/expedientes', {
                numero,
                unidad: unidad || null,
                descripcion: descripcion || null,
                anio: anio || null
            });
            app.avisar('Expediente registrado y asignado a tu custodia');
            agregando = false;
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            guardando = false;
        }
    }

    async function cargarPendientes() {
        try {
            const respuesta = await api<{ transferencias: any[] }>('GET', '/api/v1/transferencias/pendientes');
            pendientes = respuesta.transferencias;
        } catch {
            pendientes = [];
        }
    }

    async function aceptar(id: string) {
        try {
            await api('POST', `/api/v1/transferencias/${id}/aceptar`, {});
            app.avisar('Transferencia aceptada; el expediente está en tu custodia');
            await cargar();
            await cargarPendientes();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function rechazar(id: string) {
        try {
            await api('POST', `/api/v1/transferencias/${id}/rechazar`, {});
            app.avisar('Transferencia rechazada');
            await cargarPendientes();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function devolver(expedienteId: string) {
        try {
            await api('POST', '/api/v1/devoluciones', {
                expediente_id: expedienteId,
                observaciones: null
            });
            app.avisar('Devolución solicitada; la custodia sigue contigo hasta el acuse del almacén');
            await cargar();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function abrirTransferencia(custodia: any) {
        transferencia = custodia;
        destino = '';
        observaciones = '';
        try {
            const respuesta = await api<{ usuarios: any[] }>('GET', '/api/v1/usuarios/directorio');
            directorio = respuesta.usuarios;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function transferir() {
        if (!transferencia || !destino) return;
        try {
            await api('POST', '/api/v1/transferencias', {
                expediente_id: transferencia.expediente_id,
                destino_id: destino,
                observaciones: observaciones || null
            });
            app.avisar('Transferencia solicitada; el receptor debe aceptarla');
            transferencia = null;
            await cargar();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="text-sm text-slate-500">Expedientes bajo mi custodia</p>
        <div class="flex gap-2">
            {#if app.permiso('expedientes:crear')}
                <button class="btn-primario" onclick={abrirAlta}>Agregar expediente</button>
            {/if}
            <button class="btn-secundario" onclick={cargar}>Actualizar</button>
        </div>
    </div>

    {#if pendientes.length > 0}
        <section class="tarjeta overflow-hidden">
            <header class="flex items-center justify-between border-b border-slate-100 px-4 py-3">
                <h2 class="text-sm font-semibold text-slate-700">Transferencias por aceptar</h2>
                <span class="insignia bg-amber-100 text-amber-700">{pendientes.length}</span>
            </header>
            <div class="overflow-x-auto"><table class="tabla min-w-[720px]">
                <thead>
                    <tr>
                        <th>Expediente</th>
                        <th>De</th>
                        <th>Solicitada</th>
                        <th class="text-right">Acciones</th>
                    </tr>
                </thead>
                <tbody>
                    {#each pendientes as pendiente}
                        <tr>
                            <td class="font-medium">{pendiente.numero}</td>
                            <td class="text-slate-600">{pendiente.origen}</td>
                            <td class="text-slate-500">
                                {new Date(pendiente.solicitada_en).toLocaleString('es-MX')}
                            </td>
                            <td class="space-x-2 text-right">
                                <button class="btn-primario" onclick={() => aceptar(pendiente.id)}>
                                    Aceptar
                                </button>
                                <button class="btn-secundario" onclick={() => rechazar(pendiente.id)}>
                                    Rechazar
                                </button>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table></div>
        </section>
    {/if}

    <div class="tarjeta overflow-hidden">
        <div class="overflow-x-auto"><table class="tabla min-w-[720px]">
            <thead>
                <tr>
                    <th>Expediente</th>
                    <th>Descripción</th>
                    <th>Estado</th>
                    <th>Desde</th>
                    <th class="text-right">Acciones</th>
                </tr>
            </thead>
            <tbody>
                {#each visibles as custodia}
                    <tr>
                        <td class="font-medium">{custodia.numero}</td>
                        <td class="text-slate-600">{custodia.descripcion}</td>
                        <td><Insignia estado={custodia.estado} /></td>
                        <td class="text-slate-500">
                            {new Date(custodia.desde).toLocaleString('es-MX')}
                        </td>
                        <td class="space-x-2 text-right">
                            <button class="btn-secundario" onclick={() => devolver(custodia.expediente_id)}>
                                Devolver
                            </button>
                            <button class="btn-secundario" onclick={() => abrirTransferencia(custodia)}>
                                Transferir
                            </button>
                        </td>
                    </tr>
                {/each}
                {#if custodias.length === 0}
                    <tr><td colspan="5" class="py-6 text-center text-slate-400">
                        No tienes expedientes en custodia
                    </td></tr>
                {/if}
            </tbody>
        </table></div>
        <Paginacion {pagina} {paginas} total={custodias.length} onCambiar={(valor) => (pagina = valor)} />
    </div>
</div>

{#if agregando}
    <Modal titulo="Agregar expediente" oncerrar={() => (agregando = false)}>
        <form
            class="space-y-4"
            onsubmit={(evento) => {
                evento.preventDefault();
                registrar();
            }}
        >
            <p class="rounded-lg bg-sky-50 px-3 py-2 text-xs text-sky-800">
                El número se completa con ceros a la izquierda hasta 8 caracteres antes de la barra
                (5259/2026 → 00005259/2026). Al registrarlo, el expediente queda de inmediato bajo tu
                custodia. Para regresarlo al almacén usa «Devolver»; la custodia se cierra cuando el
                almacén acuse la recepción.
            </p>

            <div class="grid gap-3 sm:grid-cols-2">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Número de expediente</span>
                    <input class="campo" bind:value={numero} required />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Unidad / órgano</span>
                    <input class="campo" bind:value={unidad} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Año</span>
                    <input class="campo" type="number" min="1900" max="2200" bind:value={anio} />
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Descripción</span>
                    <input class="campo" bind:value={descripcion} />
                </label>
            </div>

            <button class="btn-primario" type="submit" disabled={guardando}>
                {guardando ? 'Registrando…' : 'Registrar y tomar custodia'}
            </button>
        </form>
    </Modal>
{/if}

{#if transferencia}
    <Modal titulo="Transferir custodia" oncerrar={() => (transferencia = null)}>
        <div class="space-y-4">
            <p class="text-sm text-slate-600">
                Expediente <strong>{transferencia.numero}</strong>. La custodia cambiará cuando el
                receptor acepte la transferencia.
            </p>
            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Usuario destino</span>
                <select class="campo" bind:value={destino}>
                    <option value="">Seleccione…</option>
                    {#each directorio as usuario}
                        <option value={usuario.id}>{usuario.nombre}</option>
                    {/each}
                </select>
            </label>
            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Observaciones</span>
                <input class="campo" bind:value={observaciones} />
            </label>
            <button class="btn-primario" disabled={!destino} onclick={transferir}>
                Solicitar transferencia
            </button>
        </div>
    </Modal>
{/if}
