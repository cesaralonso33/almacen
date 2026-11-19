<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { onMount, tick } from 'svelte';
    import { app } from './estado.svelte';

    let correo = $state('');
    let password = $state('');
    let servidor = $state('https://129-121-117-177.sslip.io');
    let mostrarServidor = $state(false);
    let error = $state('');

    onMount(async () => {
        try {
            servidor = await invoke<string>('servidor_actual');
        } catch {
            /* se conserva el valor por omisión */
        }
    });

    async function alternarServidor() {
        mostrarServidor = !mostrarServidor;
        if (mostrarServidor) {
            await tick();
            document.getElementById('campo-servidor')?.focus();
        }
    }

    async function entrar(evento: SubmitEvent) {
        evento.preventDefault();
        error = '';
        if (!servidor.trim()) {
            error = 'La dirección del servidor no puede estar vacía.';
            mostrarServidor = true;
            return;
        }
        try {
            await invoke('configurar_servidor', { url: servidor });
            await app.iniciarSesion(correo.trim(), password);
        } catch (e) {
            error = String(e);
        }
    }
</script>

<main class="flex min-h-screen items-center justify-center bg-gradient-to-br from-institucional-900 via-institucional-700 to-slate-900 p-3 sm:p-6">
    <div class="w-full max-w-md rounded-2xl bg-white p-5 shadow-2xl sm:p-8">
        <div class="mb-6">
            <p class="text-xs font-semibold uppercase tracking-widest text-institucional-500">
                Institución
            </p>
            <h1 class="mt-1 text-2xl font-semibold text-slate-800">Custodia CradSystem</h1>
            <p class="mt-2 text-sm text-slate-500">
                Expedientes jurídicos · acceso con credenciales institucionales
            </p>
        </div>

        <form class="space-y-4" onsubmit={entrar}>
            {#if mostrarServidor}
                <label class="block text-sm" for="campo-servidor">
                    <span class="mb-1 block font-medium text-slate-600">Servidor</span>
                    <input
                        id="campo-servidor"
                        class="campo"
                        type="url"
                        bind:value={servidor}
                        placeholder="https://129-121-117-177.sslip.io"
                        required
                    />
                </label>

                {#if servidor.startsWith('http://')}
                    <p class="rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-800" role="alert">
                        Conexión sin cifrado: el tráfico viaja en claro. Use esta dirección solo en
                        una red cerrada o VPN institucional.
                    </p>
                {/if}
            {/if}

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

    <button
        class="fixed bottom-4 right-4 rounded-full border border-white/25 bg-white/10 p-2.5 text-white/80 shadow-lg backdrop-blur transition hover:bg-white/20 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/70"
        type="button"
        onclick={alternarServidor}
        aria-controls="campo-servidor"
        aria-expanded={mostrarServidor}
        aria-label={mostrarServidor
            ? 'Ocultar configuración del servidor'
            : 'Mostrar configuración del servidor'}
        title={mostrarServidor ? 'Ocultar servidor' : 'Configurar servidor'}
    >
        <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="h-5 w-5"
            aria-hidden="true"
        >
            <line x1="4" y1="21" x2="4" y2="14" />
            <line x1="4" y1="10" x2="4" y2="3" />
            <line x1="12" y1="21" x2="12" y2="12" />
            <line x1="12" y1="8" x2="12" y2="3" />
            <line x1="20" y1="21" x2="20" y2="16" />
            <line x1="20" y1="12" x2="20" y2="3" />
            <line x1="1" y1="14" x2="7" y2="14" />
            <line x1="9" y1="8" x2="15" y2="8" />
            <line x1="17" y1="16" x2="23" y2="16" />
        </svg>
    </button>
</main>
