<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api, type Expediente } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let buscar = $state('');
    let expedientes = $state<Expediente[]>([]);
    let seleccionado = $state<any>(null);
    let historial = $state<any[]>([]);
    let local = $state(false);
    let pagina = $state(1);

    const paginas = $derived(totalPaginas(expedientes.length, POR_PAGINA));
    const visibles = $derived(recortar(expedientes, pagina, POR_PAGINA));

    let creando = $state(false);
    let numeroNuevo = $state('');
    let descripcionNueva = $state('');
    let unidadNueva = $state('');
    let anioNuevo = $state<number | null>(null);
    let guardando = $state(false);

    let temporizador: ReturnType<typeof setTimeout>;

    onMount(cargar);

    function conRetardo() {
        clearTimeout(temporizador);
        temporizador = setTimeout(cargar, 250);
    }

    async function cargar() {
        try {
            const respuesta = await api<{ expedientes: Expediente[] }>(
                'GET',
                `/api/v1/expedientes?buscar=${encodeURIComponent(buscar)}`
            );
            expedientes = respuesta.expedientes;
            local = false;
            pagina = 1;
        } catch {
            // Sin conexión: se consulta la proyección local autorizada previamente sincronizada.
            try {
                const cache = await invoke<any[]>('listar_expedientes', { filtro: buscar });
                expedientes = cache.map((item) => ({
                    ...item,
                    custodio: item.custodio_id ? 'En custodia' : null
                }));
                pagina = 1;
                local = true;
            } catch (error) {
                app.avisar(String(error), 'error');
            }
        }
    }

    async function crearExpediente() {
        guardando = true;
        try {
            await api('POST', '/api/v1/expedientes', {
                numero: numeroNuevo,
                descripcion: descripcionNueva || null,
                unidad: unidadNueva || null,
                anio: anioNuevo || null
            });
            app.avisar('Expediente registrado');
            creando = false;
            numeroNuevo = '';
            descripcionNueva = '';
            unidadNueva = '';
            anioNuevo = null;
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            guardando = false;
        }
    }

    async function abrir(expediente: Expediente) {
        try {
            const respuesta = await api<any>('GET', `/api/v1/expedientes/${expediente.id}/historial`);
            seleccionado = respuesta.expediente;
            historial = respuesta.movimientos;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-center gap-3">
        <input
            class="campo max-w-md"
            placeholder="Buscar por número o descripción…"
            bind:value={buscar}
            oninput={conRetardo}
        />
        <button class="btn-secundario" onclick={cargar}>Actualizar</button>
        {#if app.permiso('expedientes:administrar')}
            <button class="btn-primario" onclick={() => (creando = true)}>Nuevo expediente</button>
        {/if}
        <span class="text-sm text-slate-500">{expedientes.length} resultados</span>
    </div>

    {#if local}
        <p class="rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-700">
            Sin conexión: mostrando la copia local de expedientes. La información puede estar
            desactualizada.
        </p>
    {/if}

    <div class="tarjeta overflow-hidden">
        <div class="max-h-[65vh] overflow-auto">
            <table class="tabla">
                <thead>
                    <tr>
                        <th>Número</th>
                        <th>Descripción</th>
                        <th>Estado</th>
                        <th>Custodio</th>
                        <th>Versión</th>
                    </tr>
                </thead>
                <tbody>
                    {#each visibles as expediente}
                        <tr
                            class="cursor-pointer"
                            onclick={() => abrir(expediente)}
                            onkeydown={(e) => e.key === 'Enter' && abrir(expediente)}
                            tabindex="0"
                        >
                            <td class="font-medium text-institucional-600">{expediente.numero}</td>
                            <td class="text-slate-600">{expediente.descripcion}</td>
                            <td><Insignia estado={expediente.estado} /></td>
                            <td class="text-slate-500">{expediente.custodio ?? '—'}</td>
                            <td class="text-slate-400">{expediente.version}</td>
                        </tr>
                    {/each}
                    {#if expedientes.length === 0}
                        <tr><td colspan="5" class="py-6 text-center text-slate-400">Sin resultados</td></tr>
                    {/if}
                </tbody>
            </table>
        </div>
        <Paginacion
            {pagina}
            {paginas}
            total={expedientes.length}
            onCambiar={(valor) => (pagina = valor)}
        />
    </div>
</div>

{#if creando}
    <Modal titulo="Nuevo expediente" oncerrar={() => (creando = false)}>
        <form
            class="space-y-4"
            onsubmit={(evento) => {
                evento.preventDefault();
                crearExpediente();
            }}
        >
            <div class="grid gap-3 sm:grid-cols-2">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Número de expediente</span>
                    <input class="campo" bind:value={numeroNuevo} required />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Unidad / órgano</span>
                    <input class="campo" bind:value={unidadNueva} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Año</span>
                    <input class="campo" type="number" min="1900" max="2200" bind:value={anioNuevo} />
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Descripción</span>
                    <input class="campo" bind:value={descripcionNueva} />
                </label>
            </div>

            <p class="text-xs text-slate-500">
                El número es único por unidad y año. El expediente se registra en almacén.
            </p>

            <button class="btn-primario" type="submit" disabled={guardando}>
                {guardando ? 'Guardando…' : 'Registrar expediente'}
            </button>
        </form>
    </Modal>
{/if}

{#if seleccionado}
    <Modal titulo={`Expediente ${seleccionado.numero}`} oncerrar={() => (seleccionado = null)}>
        <dl class="mb-4 grid grid-cols-2 gap-3 text-sm">
            <div><dt class="text-slate-500">Estado</dt><dd><Insignia estado={seleccionado.estado} /></dd></div>
            <div><dt class="text-slate-500">Custodio actual</dt><dd>{seleccionado.custodio ?? 'Almacén'}</dd></div>
        </dl>
        <h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
            Historial de movimientos
        </h3>
        <ul class="space-y-2">
            {#each historial as movimiento}
                <li class="rounded-lg border border-slate-100 px-3 py-2 text-sm">
                    <div class="flex justify-between">
                        <span class="font-medium text-slate-700">{movimiento.tipo.replaceAll('_', ' ')}</span>
                        <span class="text-xs text-slate-400">
                            {new Date(movimiento.creado_en).toLocaleString('es-MX')}
                        </span>
                    </div>
                    <p class="text-xs text-slate-500">
                        {movimiento.actor}{movimiento.detalle ? ` · ${movimiento.detalle}` : ''}
                    </p>
                </li>
            {/each}
            {#if historial.length === 0}
                <li class="text-sm text-slate-400">Sin movimientos registrados</li>
            {/if}
        </ul>
    </Modal>
{/if}
