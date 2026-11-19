<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let devoluciones = $state<any[]>([]);
    let pagina = $state(1);

    const paginas = $derived(totalPaginas(devoluciones.length, POR_PAGINA));
    const visibles = $derived(recortar(devoluciones, pagina, POR_PAGINA));

    onMount(cargar);

    async function cargar() {
        try {
            const respuesta = await api<{ devoluciones: any[] }>('GET', '/api/v1/devoluciones');
            devoluciones = respuesta.devoluciones;
            pagina = 1;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function confirmar(id: string) {
        try {
            await api('POST', `/api/v1/devoluciones/${id}/confirmar`, {});
            app.avisar('Recepción confirmada; la custodia queda cerrada');
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-4">
    <div class="flex items-center justify-between">
        <p class="text-sm text-slate-500">Devoluciones pendientes de acuse en almacén</p>
        <button class="btn-secundario" onclick={cargar}>Actualizar</button>
    </div>

    <div class="tarjeta overflow-hidden">
        <div class="overflow-x-auto"><table class="tabla min-w-[640px]">
            <thead>
                <tr>
                    <th>Expediente</th>
                    <th>Custodio</th>
                    <th>Solicitada</th>
                    <th>Observaciones</th>
                    <th></th>
                </tr>
            </thead>
            <tbody>
                {#each visibles as devolucion}
                    <tr>
                        <td class="font-medium">{devolucion.numero}</td>
                        <td class="text-slate-600">{devolucion.custodio}</td>
                        <td class="text-slate-500">
                            {new Date(devolucion.solicitada_en).toLocaleString('es-MX')}
                        </td>
                        <td class="text-slate-500">{devolucion.observaciones ?? '—'}</td>
                        <td class="text-right">
                            <button class="btn-primario" onclick={() => confirmar(devolucion.id)}>
                                Acusar recepción
                            </button>
                        </td>
                    </tr>
                {/each}
                {#if devoluciones.length === 0}
                    <tr><td colspan="5" class="py-6 text-center text-slate-400">
                        No hay devoluciones pendientes
                    </td></tr>
                {/if}
            </tbody>
        </table></div>
    </div>
    <Paginacion {pagina} {paginas} total={devoluciones.length} onCambiar={(valor) => (pagina = valor)} />
</div>
