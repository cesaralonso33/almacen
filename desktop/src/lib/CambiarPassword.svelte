<script lang="ts">
    import { api } from './api';
    import { app } from './estado.svelte';
    import Modal from './Modal.svelte';

    let { oncerrar }: { oncerrar: () => void } = $props();

    let actual = $state('');
    let nueva = $state('');
    let confirmar = $state('');
    let error = $state('');
    let ocupado = $state(false);

    async function guardar(evento: SubmitEvent) {
        evento.preventDefault();
        error = '';

        if (nueva.length < 12) {
            error = 'La nueva contraseña debe tener al menos 12 caracteres';
            return;
        }
        if (nueva !== confirmar) {
            error = 'La confirmación no coincide';
            return;
        }
        if (nueva === actual) {
            error = 'La nueva contraseña debe ser distinta de la actual';
            return;
        }

        ocupado = true;
        try {
            await api('POST', '/api/v1/auth/password', { actual, nueva });
            app.avisar('Contraseña actualizada; inicie sesión de nuevo');
            app.cerrarSesionLocal();
        } catch (e) {
            error = String(e);
            ocupado = false;
        }
    }
</script>

<Modal titulo="Cambiar contraseña" {oncerrar}>
    <form class="space-y-4" onsubmit={guardar}>
        <label class="block text-sm">
            <span class="mb-1 block font-medium text-slate-600">Contraseña actual</span>
            <input class="campo" type="password" autocomplete="current-password" bind:value={actual} required />
        </label>

        <label class="block text-sm">
            <span class="mb-1 block font-medium text-slate-600">Nueva contraseña</span>
            <input class="campo" type="password" autocomplete="new-password" bind:value={nueva} required />
            <span class="mt-1 block text-xs text-slate-500">Mínimo 12 caracteres.</span>
        </label>

        <label class="block text-sm">
            <span class="mb-1 block font-medium text-slate-600">Confirmar nueva contraseña</span>
            <input class="campo" type="password" autocomplete="new-password" bind:value={confirmar} required />
        </label>

        {#if error}
            <p class="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700" role="alert">{error}</p>
        {/if}

        <p class="text-xs text-slate-500">
            Al cambiar la contraseña se cierran todas tus sesiones, incluida esta.
        </p>

        <button class="btn-primario" type="submit" disabled={ocupado}>
            {ocupado ? 'Guardando…' : 'Cambiar contraseña'}
        </button>
    </form>
</Modal>
