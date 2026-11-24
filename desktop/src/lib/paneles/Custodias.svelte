<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import BotonIcono from '../BotonIcono.svelte';
    import Icono from '../Icono.svelte';
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
    let buscar = $state('');

    let agregando = $state(false);
    let numero = $state('');
    let unidad = $state('');
    let anio = $state<number | null>(new Date().getFullYear());
    let descripcion = $state('');
    let guardando = $state(false);

    function normalizar(valor: unknown): string {
        return String(valor ?? '')
            .toLocaleLowerCase('es-MX')
            .normalize('NFD')
            .replace(/[\u0300-\u036f]/g, '');
    }

    const custodiasFiltradas = $derived.by(() => {
        const termino = normalizar(buscar).trim();
        if (!termino) return custodias;
        return custodias.filter((custodia) =>
            [custodia.numero, custodia.descripcion, custodia.estado, custodia.unidad]
                .map((valor) => normalizar(valor))
                .some((valor) => valor.includes(termino))
        );
    });
    const paginas = $derived(totalPaginas(custodiasFiltradas.length, POR_PAGINA));
    const visibles = $derived(recortar(custodiasFiltradas, pagina, POR_PAGINA));

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
        <p class="text-sm text-slate-500">
            {custodiasFiltradas.length} expedientes bajo mi custodia
        </p>
        <div class="flex items-center gap-1">
            <BotonIcono nombre="actualizar" etiqueta="Actualizar" onclick={cargar} />
            {#if app.permiso('expedientes:crear')}
                <button class="btn-primario ml-1 px-3 py-1.5 text-xs" onclick={abrirAlta}>
                    <Icono nombre="mas" clase="h-3.5 w-3.5" />
                    Agregar expediente
                </button>
            {/if}
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
                            <td class="text-right">
                                <div class="flex justify-end gap-1">
                                    <button
                                        class="btn-primario px-2.5 py-1 text-xs"
                                        onclick={() => aceptar(pendiente.id)}
                                    >
                                        Aceptar
                                    </button>
                                    <button
                                        class="btn-secundario px-2.5 py-1 text-xs"
                                        onclick={() => rechazar(pendiente.id)}
                                    >
                                        Rechazar
                                    </button>
                                </div>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table></div>
        </section>
    {/if}

    <div class="tarjeta overflow-hidden">
        <div class="barra-herramientas">
            <div class="relative min-w-0 flex-1 sm:max-w-sm">
                <Icono
                    nombre="buscar"
                    clase="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400"
                />
                <input
                    class="campo border-transparent bg-slate-50 pl-9 focus:bg-white"
                    type="search"
                    placeholder="Buscar por número, descripción o estado…"
                    aria-label="Buscar en mis custodias"
                    bind:value={buscar}
                    oninput={() => (pagina = 1)}
                />
            </div>
        </div>
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
                            <td class="text-right">
                                <div class="flex justify-end gap-1">
                                    <BotonIcono
                                        nombre="devolver"
                                        etiqueta={`Devolver ${custodia.numero}`}
                                        chico
                                        onclick={() => devolver(custodia.expediente_id)}
                                    />
                                    <BotonIcono
                                        nombre="transferir"
                                        etiqueta={`Transferir ${custodia.numero}`}
                                        chico
                                        onclick={() => abrirTransferencia(custodia)}
                                    />
                                </div>
                            </td>
                    </tr>
                {/each}
                {#if custodiasFiltradas.length === 0}
                    <tr><td colspan="5" class="py-6 text-center text-slate-400">
                        {custodias.length === 0
                            ? 'No tienes expedientes en custodia'
                            : 'No hay custodias que coincidan con la búsqueda'}
                    </td></tr>
                {/if}
            </tbody>
        </table></div>
        <Paginacion {pagina} {paginas} total={custodiasFiltradas.length} onCambiar={(valor) => (pagina = valor)} />
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
