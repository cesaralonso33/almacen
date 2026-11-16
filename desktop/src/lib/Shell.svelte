<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { app } from './estado.svelte';
    import Modal from './Modal.svelte';
    import Inicio from './paneles/Inicio.svelte';
    import Expedientes from './paneles/Expedientes.svelte';
    import Pedidos from './paneles/Pedidos.svelte';
    import Almacen from './paneles/Almacen.svelte';
    import Entregas from './paneles/Entregas.svelte';
    import Custodias from './paneles/Custodias.svelte';
    import Devoluciones from './paneles/Devoluciones.svelte';
    import Reportes from './paneles/Reportes.svelte';
    import Admin from './paneles/Admin.svelte';

    const items = [
        { id: 'inicio', etiqueta: 'Inicio', permiso: null },
        { id: 'expedientes', etiqueta: 'Expedientes', permiso: 'expedientes:leer' },
        { id: 'pedidos', etiqueta: 'Mis pedidos', permiso: 'pedidos:leer' },
        { id: 'almacen', etiqueta: 'Almacén', permiso: 'pedidos:preparar' },
        { id: 'entregas', etiqueta: 'Entregas', permiso: 'entregas:confirmar' },
        { id: 'custodias', etiqueta: 'Mis custodias', permiso: 'expedientes:leer' },
        { id: 'devoluciones', etiqueta: 'Devoluciones', permiso: 'devoluciones:confirmar' },
        { id: 'reportes', etiqueta: 'Reportes', permiso: 'reportes:leer' },
        { id: 'admin', etiqueta: 'Administración', permiso: 'admin:usuarios' }
    ];

    let conflictos = $state<any[]>([]);
    let verConflictos = $state(false);

    const titulo = $derived(items.find((item) => item.id === app.panel)?.etiqueta ?? 'Inicio');
    const estadoSync = $derived(app.sync?.estado ?? 'SIN_CONEXION');
    const claseSync = $derived(
        estadoSync === 'EN_LINEA'
            ? 'bg-emerald-100 text-emerald-700'
            : estadoSync === 'ERROR'
              ? 'bg-rose-100 text-rose-700'
              : 'bg-slate-200 text-slate-600'
    );

    async function abrirConflictos() {
        try {
            conflictos = await invoke<any[]>('listar_conflictos');
            verConflictos = true;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="flex h-screen overflow-hidden">
    <aside class="flex w-60 shrink-0 flex-col bg-institucional-900 text-slate-200">
        <div class="px-5 py-5">
            <p class="text-[10px] font-semibold uppercase tracking-[0.2em] text-institucional-100/70">
                Institución
            </p>
            <h1 class="mt-1 text-base font-semibold text-white">Almacén y custodia</h1>
        </div>

        <nav class="flex-1 space-y-1 px-3" aria-label="Navegación principal">
            {#each items as item}
                {#if !item.permiso || app.permiso(item.permiso)}
                    <button
                        class="w-full rounded-lg px-3 py-2 text-left text-sm transition {app.panel === item.id
                            ? 'bg-white/10 font-semibold text-white'
                            : 'text-slate-300 hover:bg-white/5'}"
                        onclick={() => (app.panel = item.id)}
                        aria-current={app.panel === item.id ? 'page' : undefined}
                    >
                        {item.etiqueta}
                    </button>
                {/if}
            {/each}
        </nav>

        <div class="border-t border-white/10 px-4 py-4 text-sm">
            <p class="truncate font-medium text-white">{app.usuario?.nombre}</p>
            <button class="mt-2 text-xs text-slate-300 underline" onclick={() => app.salir()}>
                Cerrar sesión
            </button>
        </div>
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
        <header class="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 bg-white px-6 py-3">
            <h2 class="text-lg font-semibold text-slate-800">{titulo}</h2>

            <div class="flex items-center gap-3">
                {#if app.inseguro}
                    <span
                        class="insignia bg-amber-100 text-amber-800"
                        title="La conexión no está cifrada. Use solo en red cerrada o VPN."
                    >
                        Sin cifrado
                    </span>
                {/if}
                <span class="insignia {claseSync}">
                    {estadoSync === 'EN_LINEA' ? 'En línea' : estadoSync === 'ERROR' ? 'Error de sincronización' : 'Sin conexión'}
                </span>

                {#if app.sync && app.sync.pendientes > 0}
                    <span class="text-xs text-slate-500">{app.sync.pendientes} pendientes</span>
                {/if}

                {#if app.sync && app.sync.conflictos > 0}
                    <button class="btn-secundario" onclick={abrirConflictos}>
                        {app.sync.conflictos} conflictos
                    </button>
                {/if}

                <button class="btn-primario" disabled={app.ocupado} onclick={() => app.sincronizar()}>
                    {app.ocupado ? 'Sincronizando…' : 'Sincronizar'}
                </button>
            </div>
        </header>

        <main class="flex-1 overflow-y-auto p-6">
            {#if app.panel === 'inicio'}<Inicio />{/if}
            {#if app.panel === 'expedientes'}<Expedientes />{/if}
            {#if app.panel === 'pedidos'}<Pedidos />{/if}
            {#if app.panel === 'almacen'}<Almacen />{/if}
            {#if app.panel === 'entregas'}<Entregas />{/if}
            {#if app.panel === 'custodias'}<Custodias />{/if}
            {#if app.panel === 'devoluciones'}<Devoluciones />{/if}
            {#if app.panel === 'reportes'}<Reportes />{/if}
            {#if app.panel === 'admin'}<Admin />{/if}
        </main>
    </div>
</div>

{#if verConflictos}
    <Modal titulo="Conflictos de sincronización" oncerrar={() => (verConflictos = false)}>
        {#if conflictos.length === 0}
            <p class="text-sm text-slate-500">No hay conflictos pendientes.</p>
        {:else}
            <ul class="space-y-2">
                {#each conflictos as conflicto}
                    <li class="rounded-lg border border-rose-100 bg-rose-50 px-3 py-2 text-sm">
                        <p class="font-medium text-rose-700">{conflicto.tipo ?? 'Operación'}</p>
                        <p class="text-xs text-rose-600">{conflicto.motivo}</p>
                        <p class="mt-1 text-[11px] text-slate-500">{conflicto.creado_en}</p>
                    </li>
                {/each}
            </ul>
            <p class="mt-4 text-xs text-slate-500">
                Un conflicto significa que el servidor rechazó la operación local; requiere revisión
                humana. No se reenvía automáticamente.
            </p>
        {/if}
    </Modal>
{/if}
