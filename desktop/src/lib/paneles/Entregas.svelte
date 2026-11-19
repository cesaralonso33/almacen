<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let entregas = $state<any[]>([]);
    let pagina = $state(1);

    const paginas = $derived(totalPaginas(entregas.length, POR_PAGINA));
    const visibles = $derived(recortar(entregas, pagina, POR_PAGINA));

    onMount(cargar);

    async function cargar() {
        try {
            const respuesta = await api<{ entregas: any[] }>('GET', '/api/v1/entregas/pendientes');
            entregas = respuesta.entregas;
            pagina = 1;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function confirmar(id: string) {
        try {
            await api('POST', `/api/v1/entregas/${id}/confirmar`, {});
            app.avisar('Recepción confirmada');
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-4">
    <div class="flex items-center justify-between">
        <p class="text-sm text-slate-500">Entregas pendientes de mi acuse</p>
        <button class="btn-secundario" onclick={cargar}>Actualizar</button>
    </div>

    <div class="tarjeta overflow-hidden">
        <div class="overflow-x-auto"><table class="tabla min-w-[640px]">
            <thead>
                <tr>
                    <th>Expediente</th>
                    <th>Descripción</th>
                    <th>Entregó</th>
                    <th>Despachada</th>
                    <th></th>
                </tr>
            </thead>
            <tbody>
                {#each visibles as entrega}
                    <tr>
                        <td class="font-medium">{entrega.numero}</td>
                        <td class="text-slate-600">{entrega.descripcion}</td>
                        <td class="text-slate-500">{entrega.entregador}</td>
                        <td class="text-slate-500">
                            {new Date(entrega.creada_en).toLocaleString('es-MX')}
                        </td>
                        <td class="text-right">
                            <button class="btn-primario" onclick={() => confirmar(entrega.id)}>
                                Confirmar recepción
                            </button>
                        </td>
                    </tr>
                {/each}
                {#if entregas.length === 0}
                    <tr><td colspan="5" class="py-6 text-center text-slate-400">
                        No hay entregas pendientes de acuse
                    </td></tr>
                {/if}
            </tbody>
        </table></div>
    </div>
    <Paginacion {pagina} {paginas} total={entregas.length} onCambiar={(valor) => (pagina = valor)} />
</div>
