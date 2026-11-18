<script lang="ts">
    import { onMount } from 'svelte';
    import { check } from '@tauri-apps/plugin-updater';
    import { relaunch } from '@tauri-apps/plugin-process';

    let actualizacion = $state<any>(null);
    let instalando = $state(false);
    let progreso = $state(0);
    let error = $state('');

    onMount(() => {
        buscar();
        const temporizador = window.setInterval(buscar, 6 * 60 * 60 * 1000);
        return () => window.clearInterval(temporizador);
    });

    async function buscar() {
        try {
            actualizacion = await check();
        } catch {
            // Sin conexión o servidor de actualizaciones no disponible: no bloquear la app.
        }
    }

    async function instalar() {
        if (!actualizacion || instalando) return;
        instalando = true;
        error = '';
        progreso = 0;
        try {
            await actualizacion.downloadAndInstall((evento: any) => {
                if (evento.event === 'Started') progreso = 0;
                if (evento.event === 'Progress') progreso += evento.data.chunkLength;
            });
            await relaunch();
        } catch (fallo) {
            error = String(fallo);
        } finally {
            instalando = false;
        }
    }
</script>

{#if actualizacion}
    <div
        class="fixed bottom-5 left-5 z-50 max-w-md rounded-lg border border-indigo-200 bg-white px-4 py-3 text-sm shadow-xl"
        role="status"
        aria-live="polite"
    >
        <p class="font-semibold text-slate-800">
            Actualización {actualizacion.version} disponible
        </p>
        {#if actualizacion.body}
            <p class="mt-1 text-slate-600">{actualizacion.body}</p>
        {/if}
        {#if error}
            <p class="mt-2 text-rose-600">{error}</p>
        {/if}
        <div class="mt-3 flex items-center gap-3">
            <button class="btn-primario" type="button" onclick={instalar} disabled={instalando}>
                {instalando ? `Descargando… ${Math.round(progreso / 1024)} KB` : 'Actualizar ahora'}
            </button>
            <button class="btn-secundario" type="button" onclick={() => (actualizacion = null)} disabled={instalando}>
                Después
            </button>
        </div>
        <p class="mt-2 text-xs text-slate-500">
            La aplicación se reiniciará. Guarde cualquier operación en curso antes de continuar.
        </p>
    </div>
{/if}
