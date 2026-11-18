<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type PedidoResumen } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { allSettledConcurrent } from '../allSettledConcurrent.js';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let tablero = $state<Record<string, number> | null>(null);
    let pedidos = $state<PedidoResumen[]>([]);
    let altas = $state<any>(null);
    let agrupacion = $state<'por_dia' | 'por_mes' | 'por_anio'>('por_mes');
    let paginaAltas = $state(1);
    let pagina = $state(1);

    const agrupaciones: Array<{ clave: 'por_dia' | 'por_mes' | 'por_anio'; etiqueta: string }> = [
        { clave: 'por_dia', etiqueta: 'Día' },
        { clave: 'por_mes', etiqueta: 'Mes' },
        { clave: 'por_anio', etiqueta: 'Año' }
    ];

    const paginas = $derived(totalPaginas(pedidos.length, POR_PAGINA));
    const visibles = $derived(recortar(pedidos, pagina, POR_PAGINA));

    const filasAltasTodas = $derived<any[]>(altas ? (altas[agrupacion] ?? []) : []);
    const paginasAltas = $derived(totalPaginas(filasAltasTodas.length, POR_PAGINA));
    const filasAltas = $derived(recortar(filasAltasTodas, paginaAltas, POR_PAGINA));

    function cambiarAgrupacion(clave: 'por_dia' | 'por_mes' | 'por_anio') {
        agrupacion = clave;
        paginaAltas = 1;
    }

    onMount(cargar);

    async function cargar() {
        const [resultadoTablero, resultadoPedidos, resultadoAltas] = await allSettledConcurrent(
            () => api<Record<string, number>>('GET', '/api/v1/dashboard'),
            () => api<{ pedidos: PedidoResumen[] }>('GET', '/api/v1/pedidos'),
            () => api('GET', '/api/v1/reportes/altas')
        );

        if (resultadoTablero.status === 'fulfilled') {
            tablero = resultadoTablero.value;
        }
        if (resultadoPedidos.status === 'fulfilled') {
            pedidos = resultadoPedidos.value.pedidos;
            pagina = 1;
        }
        paginaAltas = 1;

        if (resultadoAltas.status === 'fulfilled') {
            altas = resultadoAltas.value;
        } else {
            altas = null;
        }

        const error = [resultadoTablero, resultadoPedidos].find(
            (resultado) => resultado.status === 'rejected'
        );
        if (error?.status === 'rejected') app.avisar(String(error.reason), 'error');
    }

    const porEstado = $derived(
        pedidos.reduce<Record<string, number>>((acumulado, pedido) => {
            acumulado[pedido.estado] = (acumulado[pedido.estado] ?? 0) + 1;
            return acumulado;
        }, {})
    );
</script>

<div class="space-y-6">
    <section class="grid gap-4 sm:grid-cols-3">
        {#each Object.entries(tablero ?? {}) as [clave, valor]}
            <div class="tarjeta p-4">
                <p class="text-xs uppercase tracking-wide text-slate-500">
                    {clave.replaceAll('_', ' ')}
                </p>
                <p class="mt-1 text-2xl font-semibold text-institucional-600">{valor}</p>
            </div>
        {/each}
    </section>

    <section class="tarjeta p-4">
        <h2 class="mb-3 text-sm font-semibold text-slate-700">Pedidos por estado</h2>
        <div class="flex flex-wrap gap-3">
            {#each Object.entries(porEstado) as [estado, cantidad]}
                <div class="flex items-center gap-2 rounded-lg border border-slate-200 px-3 py-2">
                    <Insignia {estado} />
                    <span class="text-sm font-semibold text-slate-700">{cantidad}</span>
                </div>
            {/each}
            {#if Object.keys(porEstado).length === 0}
                <p class="text-sm text-slate-400">Sin pedidos registrados</p>
            {/if}
        </div>
    </section>

    <section class="tarjeta p-4">
        <div class="mb-3 flex flex-wrap items-center justify-between gap-2">
            <div>
                <h2 class="text-sm font-semibold text-slate-700">Expedientes nuevos por usuario</h2>
                <p class="text-xs text-slate-500">
                    Altas con custodia inicial · {altas?.total ?? 0} en total
                </p>
            </div>
            <div class="flex gap-1">
                {#each agrupaciones as opcion}
                    <button
                        class="btn-secundario {agrupacion === opcion.clave
                            ? 'bg-slate-100 font-semibold text-slate-900'
                            : ''}"
                        onclick={() => cambiarAgrupacion(opcion.clave)}
                    >
                        {opcion.etiqueta}
                    </button>
                {/each}
            </div>
        </div>

        <table class="tabla">
            <thead>
                <tr>
                    <th>Periodo</th>
                    <th>Usuario</th>
                    <th class="text-right">Expedientes</th>
                </tr>
            </thead>
            <tbody>
                {#each filasAltas as fila}
                    <tr>
                        <td class="font-mono text-xs text-slate-600">{fila.periodo}</td>
                        <td>{fila.usuario}</td>
                        <td class="text-right font-semibold">{fila.total}</td>
                    </tr>
                {/each}
                {#if filasAltasTodas.length === 0}
                    <tr>
                        <td colspan="3" class="py-4 text-center text-slate-400">
                            Sin altas registradas
                        </td>
                    </tr>
                {/if}
            </tbody>
        </table>
        <Paginacion
            pagina={paginaAltas}
            paginas={paginasAltas}
            total={filasAltasTodas.length}
            onCambiar={(valor) => (paginaAltas = valor)}
        />
    </section>

    <section class="tarjeta overflow-hidden">
        <header class="border-b border-slate-100 px-4 py-3">
            <h2 class="text-sm font-semibold text-slate-700">Detalle de pedidos</h2>
        </header>
        <div class="max-h-[50vh] overflow-auto">
            <table class="tabla">
                <thead>
                    <tr>
                        <th>Pedido</th>
                        <th>Solicitante</th>
                        <th>Estado</th>
                        <th>Total</th>
                        <th>Entregados</th>
                        <th>Descartados</th>
                    </tr>
                </thead>
                <tbody>
                    {#each visibles as pedido}
                        <tr>
                            <td class="font-mono text-xs text-slate-500">{pedido.id.slice(0, 8)}</td>
                            <td>{pedido.solicitante_nombre}</td>
                            <td><Insignia estado={pedido.estado} /></td>
                            <td>{pedido.total}</td>
                            <td>{pedido.entregados}</td>
                            <td>{pedido.descartados}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
        <Paginacion {pagina} {paginas} total={pedidos.length} onCambiar={(valor) => (pagina = valor)} />
    </section>
</div>
