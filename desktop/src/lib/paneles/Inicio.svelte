<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import BotonIcono from '../BotonIcono.svelte';
    import Insignia from '../Insignia.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { allSettledConcurrent } from '../allSettledConcurrent.js';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let tablero = $state<Record<string, number> | null>(null);
    let pedidos = $state<any[]>([]);
    let pagina = $state(1);
    let cargando = $state(true);

    const paginas = $derived(totalPaginas(pedidos.length, POR_PAGINA));
    const visibles = $derived(recortar(pedidos, pagina, POR_PAGINA));

    onMount(cargar);

    async function cargar() {
        cargando = true;
        try {
            const [resultadoTablero, resultadoPedidos] = await allSettledConcurrent(
                () => api<Record<string, number>>('GET', '/api/v1/dashboard'),
                () => api<{ pedidos: any[] }>('GET', '/api/v1/pedidos')
            );

            if (resultadoTablero.status === 'fulfilled') {
                tablero = resultadoTablero.value;
            }
            if (resultadoPedidos.status === 'fulfilled') {
                pedidos = resultadoPedidos.value.pedidos;
                pagina = 1;
            }

            const error = [resultadoTablero, resultadoPedidos].find(
                (resultado) => resultado.status === 'rejected'
            );
            if (error?.status === 'rejected') app.avisar(String(error.reason), 'error');
        } finally {
            cargando = false;
        }
    }

    const tarjetas = [
        { clave: 'pedidos_pendientes', titulo: 'Pedidos pendientes', color: 'text-amber-600', punto: 'bg-amber-500' },
        { clave: 'pedidos_por_preparar', titulo: 'Por preparar', color: 'text-sky-600', punto: 'bg-sky-500' },
        { clave: 'en_entrega', titulo: 'En entrega', color: 'text-purple-600', punto: 'bg-purple-500' },
        { clave: 'custodias_activas', titulo: 'Custodias activas', color: 'text-indigo-600', punto: 'bg-indigo-500' },
        { clave: 'devoluciones_pendientes', titulo: 'Devoluciones por acusar', color: 'text-rose-600', punto: 'bg-rose-500' },
        { clave: 'no_localizados', titulo: 'No localizados', color: 'text-rose-700', punto: 'bg-rose-700' }
    ];
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="text-sm text-slate-500">Resumen operativo del almacén y la cadena de custodia.</p>
        <BotonIcono
            nombre="actualizar"
            etiqueta="Actualizar resumen"
            onclick={cargar}
            disabled={cargando}
        />
    </div>

    <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {#each tarjetas as tarjeta}
            <div class="tarjeta p-4 transition hover:border-slate-300">
                <p class="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-slate-500">
                    <span class="h-1.5 w-1.5 rounded-full {tarjeta.punto}" aria-hidden="true"></span>
                    {tarjeta.titulo}
                </p>
                <p class="mt-2 text-3xl font-semibold tabular-nums {tarjeta.color}">
                    {tablero?.[tarjeta.clave] ?? (cargando ? '…' : '—')}
                </p>
            </div>
        {/each}
    </div>

    <section class="tarjeta overflow-hidden" aria-busy={cargando}>
        <header class="flex items-center justify-between gap-3 border-b border-slate-100 px-4 py-2.5">
            <h2 class="text-sm font-semibold text-slate-700">Pedidos recientes</h2>
            <BotonIcono
                nombre="actualizar"
                etiqueta="Actualizar pedidos"
                onclick={cargar}
                disabled={cargando}
            />
        </header>
        <div class="overflow-x-auto">
            <table class="tabla min-w-[640px]">
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
                        <tr>
                            <td>{pedido.solicitante_nombre}</td>
                            <td><Insignia estado={pedido.estado} /></td>
                            <td>{pedido.total}</td>
                            <td class="text-slate-500">
                                {new Date(pedido.creado_en).toLocaleString('es-MX')}
                            </td>
                        </tr>
                    {/each}
                    {#if pedidos.length === 0}
                        <tr>
                            <td colspan="4" class="py-6 text-center text-slate-400">
                                {cargando ? 'Cargando pedidos…' : 'Sin pedidos'}
                            </td>
                        </tr>
                    {/if}
                </tbody>
            </table>
        </div>
        <Paginacion {pagina} {paginas} total={pedidos.length} onCambiar={(valor) => (pagina = valor)} />
    </section>
</div>
