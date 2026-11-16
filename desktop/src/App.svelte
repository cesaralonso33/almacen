<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import Login from './lib/Login.svelte';
    import Shell from './lib/Shell.svelte';
    import { app } from './lib/estado.svelte';

    const INACTIVIDAD_MS = 30 * 60 * 1000;
    const EVENTOS_ACTIVIDAD = ['mousemove', 'keydown', 'click', 'touchstart', 'scroll'] as const;

    let ultimaActividad = Date.now();

    onMount(() => {
        const marcarActividad = () => {
            ultimaActividad = Date.now();
        };
        for (const evento of EVENTOS_ACTIVIDAD) {
            window.addEventListener(evento, marcarActividad, { passive: true });
        }

        let cerrando = false;
        const sincronizacion = window.setInterval(() => {
            if (app.usuario) app.sincronizar(true);
        }, 60_000);

        // Cierre por inactividad del puesto de trabajo. La sincronización periódica no
        // cuenta como actividad: solo el uso real del teclado o del ratón.
        const vigilante = window.setInterval(async () => {
            if (!app.usuario || cerrando) return;
            if (Date.now() - ultimaActividad < INACTIVIDAD_MS) return;

            cerrando = true;
            try {
                await invoke('logout');
            } catch {
                // La sesión ya pudo expirar en el servidor.
            }
            app.cerrarSesionLocal();
            app.avisar('Sesión cerrada por inactividad', 'error');
            ultimaActividad = Date.now();
            cerrando = false;
        }, 30_000);

        app.cargarServidor();
        app.refrescarSync();

        return () => {
            for (const evento of EVENTOS_ACTIVIDAD) {
                window.removeEventListener(evento, marcarActividad);
            }
            window.clearInterval(sincronizacion);
            window.clearInterval(vigilante);
        };
    });
</script>

{#if app.usuario}
    <Shell />
{:else}
    <Login />
{/if}

{#if app.mensaje}
    <div
        class="fixed bottom-5 right-5 z-50 max-w-sm rounded-lg px-4 py-3 text-sm text-white shadow-xl {app.mensaje
            .tipo === 'ok'
            ? 'bg-emerald-600'
            : 'bg-rose-600'}"
        role="status"
        aria-live="polite"
    >
        {app.mensaje.texto}
    </div>
{/if}
