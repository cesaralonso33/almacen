<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api, type Expediente, type PedidoResumen } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let pedidos = $state<PedidoResumen[]>([]);
    let creando = $state(false);
    let busqueda = $state('');
    let resultados = $state<Expediente[]>([]);
    let seleccion = $state<string[]>([]);
    let detalle = $state<any>(null);
    let motivoCancelacion = $state('');
    let pagina = $state(1);
    let paginaBusqueda = $state(1);

    const paginas = $derived(totalPaginas(pedidos.length, POR_PAGINA));
    const visibles = $derived(recortar(pedidos, pagina, POR_PAGINA));
    const POR_BUSQUEDA = 8;
    const paginasBusqueda = $derived(totalPaginas(resultados.length, POR_BUSQUEDA));
    const visiblesBusqueda = $derived(recortar(resultados, paginaBusqueda, POR_BUSQUEDA));

    onMount(cargar);

    async function cargar() {
        try {
            const respuesta = await api<{ pedidos: PedidoResumen[] }>('GET', '/api/v1/pedidos');
            pedidos = respuesta.pedidos;
            pagina = 1;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function buscarExpedientes() {
        try {
            const respuesta = await api<{ expedientes: Expediente[] }>(
                'GET',
                `/api/v1/expedientes?buscar=${encodeURIComponent(busqueda)}`
            );
            resultados = respuesta.expedientes;
            paginaBusqueda = 1;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    function alternar(id: string) {
        seleccion = seleccion.includes(id)
            ? seleccion.filter((valor) => valor !== id)
            : [...seleccion, id];
    }

    async function crear() {
        if (seleccion.length === 0) return;
        try {
            await api('POST', '/api/v1/pedidos', { expedientes: seleccion });
            app.avisar('Pedido creado');
            creando = false;
            seleccion = [];
            resultados = [];
            busqueda = '';
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            const mensaje = String(error);
            if (mensaje.toLowerCase().includes('sin conexión')) {
                try {
                    await invoke('crear_pedido_local', { expedientes: seleccion });
                    app.avisar(
                        'Sin conexión: pedido guardado localmente y pendiente de sincronizar',
                        'error'
                    );
                    creando = false;
                    seleccion = [];
                    resultados = [];
                    busqueda = '';
                    await app.refrescarSync();
                } catch (localError) {
                    app.avisar(String(localError), 'error');
                }
            } else {
                app.avisar(mensaje, 'error');
            }
        }
    }

    async function ver(id: string) {
        try {
            detalle = await api<any>('GET', `/api/v1/pedidos/${id}`);
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function cancelar() {
        if (!detalle) return;
        try {
            await api('POST', `/api/v1/pedidos/${detalle.id}/cancelar`, {
                motivo: motivoCancelacion
            });
            app.avisar('Pedido cancelado');
            detalle = null;
            motivoCancelacion = '';
            await cargar();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-4">
    <div class="flex items-center justify-between">
        <p class="text-sm text-slate-500">{pedidos.length} pedidos</p>
        <div class="flex gap-2">
            <button class="btn-secundario" onclick={cargar}>Actualizar</button>
            {#if app.permiso('pedidos:crear')}
                <button class="btn-primario" onclick={() => (creando = true)}>Nuevo pedido</button>
            {/if}
        </div>
    </div>

    <div class="tarjeta overflow-hidden">
        <div class="max-h-[65vh] overflow-auto">
            <table class="tabla min-w-[800px]">
                <thead>
                    <tr>
                        <th>Solicitante</th>
                        <th>Estado</th>
                        <th>Total</th>
                        <th>Entregados</th>
                        <th>Descartados</th>
                        <th>Creado</th>
                    </tr>
                </thead>
                <tbody>
                    {#each visibles as pedido}
                        <tr class="cursor-pointer" onclick={() => ver(pedido.id)}>
                            <td>{pedido.solicitante_nombre}</td>
                            <td><Insignia estado={pedido.estado} /></td>
                            <td>{pedido.total}</td>
                            <td>{pedido.entregados}</td>
                            <td>{pedido.descartados}</td>
                            <td class="text-slate-500">
                                {new Date(pedido.creado_en).toLocaleString('es-MX')}
                            </td>
                        </tr>
                    {/each}
                    {#if pedidos.length === 0}
                        <tr><td colspan="6" class="py-6 text-center text-slate-400">Sin pedidos</td></tr>
                    {/if}
                </tbody>
            </table>
        </div>
        <Paginacion {pagina} {paginas} total={pedidos.length} onCambiar={(valor) => (pagina = valor)} />
    </div>
</div>

{#if creando}
    <Modal titulo="Nuevo pedido" oncerrar={() => (creando = false)}>
        <div class="space-y-4">
            <div class="flex gap-2">
                <input
                    class="campo"
                    placeholder="Buscar expediente…"
                    bind:value={busqueda}
                    onkeydown={(e) => e.key === 'Enter' && buscarExpedientes()}
                />
                <button class="btn-secundario" onclick={buscarExpedientes}>Buscar</button>
            </div>

            <div class="max-h-72 overflow-auto rounded-lg border border-slate-200">
                <table class="tabla min-w-[800px]">
                    <tbody>
                        {#each visiblesBusqueda as expediente}
                            <tr>
                                <td class="w-8">
                                    <input
                                        type="checkbox"
                                        checked={seleccion.includes(expediente.id)}
                                        onchange={() => alternar(expediente.id)}
                                    />
                                </td>
                                <td class="font-medium">{expediente.numero}</td>
                                <td class="text-slate-600">{expediente.descripcion}</td>
                                <td><Insignia estado={expediente.estado} /></td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>

            <Paginacion
                pagina={paginaBusqueda}
                paginas={paginasBusqueda}
                total={resultados.length}
                onCambiar={(valor) => (paginaBusqueda = valor)}
            />

            <div class="flex items-center justify-between">
                <p class="text-sm text-slate-500">{seleccion.length} seleccionados</p>
                <button class="btn-primario" disabled={seleccion.length === 0} onclick={crear}>
                    Crear pedido
                </button>
            </div>
        </div>
    </Modal>
{/if}

{#if detalle}
    <Modal titulo="Detalle del pedido" oncerrar={() => (detalle = null)}>
        <div class="mb-3 flex items-center gap-3">
            <Insignia estado={detalle.estado} />
            <span class="text-sm text-slate-500">
                {detalle.entregados} entregados · {detalle.descartados} descartados ·
                {detalle.items.length} renglones
            </span>
        </div>

        <ul class="divide-y divide-slate-100 rounded-lg border border-slate-200">
            {#each detalle.items as item}
                <li class="flex items-center justify-between px-3 py-2 text-sm">
                    <div>
                        <p class="font-medium text-slate-700">{item.numero}</p>
                        <p class="text-xs text-slate-500">{item.descripcion}</p>
                    </div>
                    <Insignia estado={item.estado} />
                </li>
            {/each}
        </ul>

        {#if detalle.cancelado_motivo}
            <p class="mt-3 text-sm text-rose-600">Cancelado: {detalle.cancelado_motivo}</p>
        {/if}

        {#if detalle.estado === 'PENDIENTE' || detalle.estado === 'EN_PROCESO'}
            <div class="mt-5 space-y-2 border-t border-slate-100 pt-4">
                <label class="block text-sm font-medium text-slate-600" for="motivo-cancelar">
                    Motivo de cancelación
                </label>
                <input id="motivo-cancelar" class="campo" bind:value={motivoCancelacion} />
                <button class="btn-peligro" onclick={cancelar}>Cancelar pedido</button>
            </div>
        {/if}
    </Modal>
{/if}
