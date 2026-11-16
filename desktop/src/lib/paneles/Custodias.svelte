<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Modal from '../Modal.svelte';

    let custodias = $state<any[]>([]);
    let transferencia = $state<any>(null);
    let directorio = $state<{ id: string; nombre: string }[]>([]);
    let destino = $state('');
    let observaciones = $state('');

    let agregando = $state(false);
    let busqueda = $state('');
    let resultados = $state<any[]>([]);
    let solicitando = $state(false);

    onMount(cargar);

    async function cargar() {
        try {
            const respuesta = await api<{ custodias: any[] }>('GET', '/api/v1/custodias');
            custodias = respuesta.custodias;
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

    async function buscarExpedientes() {
        try {
            const respuesta = await api<{ expedientes: any[] }>(
                'GET',
                `/api/v1/expedientes?buscar=${encodeURIComponent(busqueda)}`
            );
            resultados = respuesta.expedientes;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function solicitar(id: string) {
        solicitando = true;
        try {
            await api('POST', '/api/v1/pedidos', { expedientes: [id] });
            app.avisar(
                'Solicitud creada. La custodia se activará cuando el almacén entregue y usted confirme.'
            );
            agregando = false;
            busqueda = '';
            resultados = [];
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            solicitando = false;
        }
    }
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="text-sm text-slate-500">Expedientes bajo mi custodia</p>
        <div class="flex gap-2">
            {#if app.permiso('pedidos:crear')}
                <button class="btn-primario" onclick={() => (agregando = true)}>
                    Agregar expediente
                </button>
            {/if}
            <button class="btn-secundario" onclick={cargar}>Actualizar</button>
        </div>
    </div>

    <div class="tarjeta overflow-hidden">
        <table class="tabla">
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
                {#each custodias as custodia}
                    <tr>
                        <td class="font-medium">{custodia.numero}</td>
                        <td class="text-slate-600">{custodia.descripcion}</td>
                        <td><Insignia estado={custodia.estado} /></td>
                        <td class="text-slate-500">
                            {new Date(custodia.desde).toLocaleString('es-MX')}
                        </td>
                        <td class="space-x-2 text-right">
                            <button class="btn-secundario" onclick={() => devolver(custodia.expediente_id)}>
                                Devolver
                            </button>
                            <button class="btn-secundario" onclick={() => abrirTransferencia(custodia)}>
                                Transferir
                            </button>
                        </td>
                    </tr>
                {/each}
                {#if custodias.length === 0}
                    <tr><td colspan="5" class="py-6 text-center text-slate-400">
                        No tienes expedientes en custodia
                    </td></tr>
                {/if}
            </tbody>
        </table>
    </div>
</div>

{#if agregando}
    <Modal titulo="Agregar expediente a mi custodia" oncerrar={() => (agregando = false)}>
        <div class="space-y-4">
            <p class="rounded-lg bg-sky-50 px-3 py-2 text-xs text-sky-800">
                La custodia no se asigna directamente. Se crea una solicitud, el almacén la prepara y
                la custodia se activa cuando usted confirma la recepción.
            </p>

            <div class="flex gap-2">
                <input
                    class="campo"
                    placeholder="Buscar por número o descripción…"
                    bind:value={busqueda}
                    onkeydown={(evento) => evento.key === 'Enter' && buscarExpedientes()}
                />
                <button class="btn-secundario" onclick={buscarExpedientes}>Buscar</button>
            </div>

            <div class="max-h-72 overflow-auto rounded-lg border border-slate-200">
                <table class="tabla">
                    <tbody>
                        {#each resultados as expediente}
                            <tr>
                                <td class="font-medium">{expediente.numero}</td>
                                <td class="text-slate-600">{expediente.descripcion}</td>
                                <td><Insignia estado={expediente.estado} /></td>
                                <td class="text-right">
                                    <button
                                        class="btn-secundario"
                                        disabled={solicitando}
                                        onclick={() => solicitar(expediente.id)}
                                    >
                                        Solicitar
                                    </button>
                                </td>
                            </tr>
                        {/each}
                        {#if resultados.length === 0}
                            <tr>
                                <td colspan="4" class="py-4 text-center text-slate-400">
                                    Busque un expediente para solicitar su entrega
                                </td>
                            </tr>
                        {/if}
                    </tbody>
                </table>
            </div>
        </div>
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
