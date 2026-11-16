<script lang="ts">
    import { app } from './estado.svelte';

    let correo = $state('');
    let password = $state('');
    let error = $state('');

    async function entrar(evento: SubmitEvent) {
        evento.preventDefault();
        error = '';
        try {
            await app.iniciarSesion(correo.trim(), password);
        } catch (e) {
            error = String(e);
        }
    }
</script>

<main class="flex min-h-screen items-center justify-center bg-gradient-to-br from-institucional-900 via-institucional-700 to-slate-900 p-6">
    <div class="w-full max-w-md rounded-2xl bg-white p-8 shadow-2xl">
        <div class="mb-6">
            <p class="text-xs font-semibold uppercase tracking-widest text-institucional-500">
                Institución
            </p>
            <h1 class="mt-1 text-2xl font-semibold text-slate-800">Almacén y custodia</h1>
            <p class="mt-2 text-sm text-slate-500">
                Expedientes jurídicos · acceso con credenciales institucionales
            </p>
        </div>

        <form class="space-y-4" onsubmit={entrar}>
            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Correo</span>
                <input
                    class="campo"
                    type="email"
                    autocomplete="username"
                    bind:value={correo}
                    placeholder="usuario@institucion.mx"
                    required
                />
            </label>

            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Contraseña</span>
                <input
                    class="campo"
                    type="password"
                    autocomplete="current-password"
                    bind:value={password}
                    required
                />
            </label>

            {#if error}
                <p class="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700" role="alert">
                    {error}
                </p>
            {/if}

            <button class="btn-primario w-full" type="submit" disabled={app.ocupado}>
                {app.ocupado ? 'Verificando…' : 'Iniciar sesión'}
            </button>
        </form>

        <p class="mt-6 text-xs text-slate-400">
            Equipo de demostración: solicitante@demo.mx · almacen@demo.mx · admin@demo.mx
        </p>
    </div>
</main>
