<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type PedidoResumen } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let pedidos = $state<PedidoResumen[]>([]);
    let detalle = $state<any>(null);
    let descartando = $state<string | null>(null);
    let motivo = $state('NO_LOCALIZADO');
    let detalleMotivo = $state('');
    let pagina = $state(1);

    const paginas = $derived(totalPaginas(pedidos.length, POR_PAGINA));
    const visibles = $derived(recortar(pedidos, pagina, POR_PAGINA));

    const motivos = [
        { clave: 'NO_LOCALIZADO', texto: 'No localizado' },
        { clave: 'NO_DISPONIBLE', texto: 'No disponible' },
        { clave: 'EN_CUSTODIA_OTRO', texto: 'En custodia de otro usuario' },
        { clave: 'REGISTRO_ERRONEO', texto: 'Registro erróneo' },
        { clave: 'OTRO', texto: 'Otro (requiere descripción)' }
    ];

    onMount(cargar);

    async function cargar() {
        try {
            const respuesta = await api<{ pedidos: PedidoResumen[] }>('GET', '/api/v1/pedidos');
            pedidos = respuesta.pedidos.filter(
                (pedido) => pedido.estado === 'PENDIENTE' || pedido.estado === 'EN_PROCESO' || pedido.estado === 'PARCIAL'
            );
            pagina = 1;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function abrir(id: string) {
        try {
            detalle = await api<any>('GET', `/api/v1/pedidos/${id}`);
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function preparar(itemId: string) {
        try {
            await api('POST', `/api/v1/items/${itemId}/preparar`, {});
            app.avisar('Renglón preparado');
            await abrir(detalle.id);
            await cargar();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function confirmarDescarte() {
        if (!descartando) return;
        try {
            await api('POST', `/api/v1/items/${descartando}/descartar`, {
                motivo_clave: motivo,
                detalle: detalleMotivo || null
            });
            app.avisar('Renglón descartado');
            descartando = null;
            detalleMotivo = '';
            motivo = 'NO_LOCALIZADO';
            await abrir(detalle.id);
            await cargar();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function despachar() {
        const items = detalle.items
            .filter((item: any) => item.estado === 'PREPARADO')
            .map((item: any) => item.id);
        if (items.length === 0) return;
        try {
            await api('POST', '/api/v1/entregas', { item_ids: items });
            app.avisar(`${items.length} renglón(es) despachados`);
            detalle = null;
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-4">
    <div class="flex items-center justify-between">
        <p class="text-sm text-slate-500">Pedidos por atender</p>
        <button class="btn-secundario" onclick={cargar}>Actualizar</button>
    </div>

    <div class="tarjeta overflow-hidden">
        <div class="overflow-x-auto"><table class="tabla min-w-[640px]">
            <thead>
                <tr>
                    <th>Solicitante</th>
                    <th>Estado</th>
                    <th>Renglones</th>
                    <th>Creado</th>
                </tr>
            </thead>
            <tbody>
                {#each visibles as pedido}
                    <tr class="cursor-pointer" onclick={() => abrir(pedido.id)}>
                        <td>{pedido.solicitante_nombre}</td>
                        <td><Insignia estado={pedido.estado} /></td>
                        <td>{pedido.total}</td>
                        <td class="text-slate-500">{new Date(pedido.creado_en).toLocaleString('es-MX')}</td>
                    </tr>
                {/each}
                {#if pedidos.length === 0}
                    <tr><td colspan="4" class="py-6 text-center text-slate-400">Sin pedidos por atender</td></tr>
                {/if}
            </tbody>
        </table></div>
    </div>
    <Paginacion {pagina} {paginas} total={pedidos.length} onCambiar={(valor) => (pagina = valor)} />
</div>

{#if detalle}
    <Modal titulo="Preparar pedido" oncerrar={() => (detalle = null)}>
        <ul class="divide-y divide-slate-100 rounded-lg border border-slate-200">
            {#each detalle.items as item}
                <li class="px-3 py-3">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <p class="font-medium text-slate-700">{item.numero}</p>
                            <p class="text-xs text-slate-500">{item.descripcion}</p>
                        </div>
                        <div class="flex items-center gap-2">
                            <Insignia estado={item.estado} />
                            {#if item.estado === 'PENDIENTE'}
                                <button class="btn-secundario" onclick={() => preparar(item.id)}>
                                    Preparar
                                </button>
                                <button class="btn-secundario" onclick={() => (descartando = item.id)}>
                                    Descartar
                                </button>
                            {/if}
                        </div>
                    </div>

                    {#if descartando === item.id}
                        <div class="mt-3 space-y-2 rounded-lg bg-slate-50 p-3">
                            <select class="campo" bind:value={motivo}>
                                {#each motivos as opcion}
                                    <option value={opcion.clave}>{opcion.texto}</option>
                                {/each}
                            </select>
                            {#if motivo === 'OTRO'}
                                <input
                                    class="campo"
                                    placeholder="Descripción obligatoria"
                                    bind:value={detalleMotivo}
                                />
                            {/if}
                            <div class="flex gap-2">
                                <button class="btn-peligro" onclick={confirmarDescarte}>Confirmar descarte</button>
                                <button class="btn-secundario" onclick={() => (descartando = null)}>
                                    Volver
                                </button>
                            </div>
                        </div>
                    {/if}
                </li>
            {/each}
        </ul>

        <div class="mt-4 flex justify-end">
            <button class="btn-primario" onclick={despachar}>Despachar preparados</button>
        </div>
    </Modal>
{/if}
