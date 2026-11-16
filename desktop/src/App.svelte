<script lang="ts">
    import { onMount } from 'svelte';
    import Login from './lib/Login.svelte';
    import Shell from './lib/Shell.svelte';
    import { app } from './lib/estado.svelte';

    onMount(() => {
        app.cargarServidor();
        app.refrescarSync();
        const intervalo = window.setInterval(() => {
            if (app.usuario) app.sincronizar(true);
        }, 60_000);
        return () => window.clearInterval(intervalo);
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
