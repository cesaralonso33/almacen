<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { allSettledConcurrent } from '../allSettledConcurrent.js';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let tablero = $state<Record<string, number> | null>(null);
    let pedidos = $state<any[]>([]);
    let pagina = $state(1);

    const paginas = $derived(totalPaginas(pedidos.length, POR_PAGINA));
    const visibles = $derived(recortar(pedidos, pagina, POR_PAGINA));

    onMount(cargar);

    async function cargar() {
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
    }

    const tarjetas = [
        { clave: 'pedidos_pendientes', titulo: 'Pedidos pendientes', color: 'text-amber-600' },
        { clave: 'pedidos_por_preparar', titulo: 'Por preparar', color: 'text-sky-600' },
        { clave: 'en_entrega', titulo: 'En entrega', color: 'text-purple-600' },
        { clave: 'custodias_activas', titulo: 'Custodias activas', color: 'text-indigo-600' },
        { clave: 'devoluciones_pendientes', titulo: 'Devoluciones por acusar', color: 'text-rose-600' },
        { clave: 'no_localizados', titulo: 'No localizados', color: 'text-rose-700' }
    ];
</script>

<div class="space-y-6">
    <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {#each tarjetas as tarjeta}
            <div class="tarjeta p-4">
                <p class="text-xs font-medium uppercase tracking-wide text-slate-500">
                    {tarjeta.titulo}
                </p>
                <p class="mt-2 text-3xl font-semibold {tarjeta.color}">
                    {tablero?.[tarjeta.clave] ?? '—'}
                </p>
            </div>
        {/each}
    </div>

    <section class="tarjeta">
        <header class="flex items-center justify-between border-b border-slate-100 px-4 py-3">
            <h2 class="text-sm font-semibold text-slate-700">Pedidos recientes</h2>
            <button class="btn-secundario" onclick={cargar}>Actualizar</button>
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
                        <tr><td colspan="4" class="py-6 text-center text-slate-400">Sin pedidos</td></tr>
                    {/if}
                </tbody>
            </table>
        </div>
        <Paginacion {pagina} {paginas} total={pedidos.length} onCambiar={(valor) => (pagina = valor)} />
    </section>
</div>
