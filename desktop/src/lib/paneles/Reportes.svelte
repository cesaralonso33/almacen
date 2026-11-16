<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type PedidoResumen } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';

    let tablero = $state<Record<string, number> | null>(null);
    let pedidos = $state<PedidoResumen[]>([]);

    onMount(cargar);

    async function cargar() {
        try {
            tablero = await api<Record<string, number>>('GET', '/api/v1/dashboard');
            const respuesta = await api<{ pedidos: PedidoResumen[] }>('GET', '/api/v1/pedidos');
            pedidos = respuesta.pedidos;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
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
                    {#each pedidos as pedido}
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
    </section>
</div>
