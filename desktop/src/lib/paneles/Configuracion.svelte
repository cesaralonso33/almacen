<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';

    type Integracion = {
        clave: string;
        url: string;
        token_configurado: boolean;
        activo: boolean;
    };

    let cargando = $state(true);
    let guardando = $state<string | null>(null);

    let urlExpediente = $state('');
    let tokenExpediente = $state('');
    let tokenExpedienteConfigurado = $state(false);
    let activoExpediente = $state(false);
    let pruebaExpediente = $state('');
    let probandoExpediente = $state(false);
    let resultadoExpediente = $state<any>(null);

    let urlPaperless = $state('');
    let tokenPaperless = $state('');
    let tokenPaperlessConfigurado = $state(false);
    let activoPaperless = $state(false);
    let probandoPaperless = $state(false);

    onMount(cargar);

    function aplicar(integracion: Integracion) {
        if (integracion.clave === 'EXPEDIENTE') {
            urlExpediente = integracion.url;
            tokenExpedienteConfigurado = integracion.token_configurado;
            activoExpediente = integracion.activo;
        } else if (integracion.clave === 'PAPERLESS') {
            urlPaperless = integracion.url;
            tokenPaperlessConfigurado = integracion.token_configurado;
            activoPaperless = integracion.activo;
        }
    }

    async function cargar() {
        cargando = true;
        try {
            const respuesta = await api<{ integraciones: Integracion[] }>(
                'GET',
                '/api/v1/integraciones'
            );
            for (const integracion of respuesta.integraciones) aplicar(integracion);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            cargando = false;
        }
    }

    async function guardar(clave: 'EXPEDIENTE' | 'PAPERLESS') {
        const esExpediente = clave === 'EXPEDIENTE';
        const url = esExpediente ? urlExpediente : urlPaperless;
        const token = esExpediente ? tokenExpediente : tokenPaperless;
        const activo = esExpediente ? activoExpediente : activoPaperless;

        guardando = clave;
        try {
            const actualizada = await api<Integracion>(
                'PUT',
                `/api/v1/integraciones/${clave}`,
                {
                    url: url.trim(),
                    token: token.trim() ? token.trim() : null,
                    activo
                }
            );
            aplicar(actualizada);
            if (esExpediente) tokenExpediente = '';
            else tokenPaperless = '';
            app.avisar('Configuración guardada');
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            guardando = null;
        }
    }

    async function probarExpediente() {
        const numero = pruebaExpediente.trim();
        if (!numero) {
            app.avisar('Capture un número de expediente para probar', 'error');
            return;
        }
        probandoExpediente = true;
        resultadoExpediente = null;
        try {
            resultadoExpediente = await api<any>(
                'POST',
                '/api/v1/integraciones/expediente/consultar',
                { expediente: numero }
            );
            app.avisar('Consulta realizada');
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            probandoExpediente = false;
        }
    }

    async function probarPaperless() {
        probandoPaperless = true;
        try {
            await api('POST', '/api/v1/integraciones/paperless/probar', {});
            app.avisar('Conexión con Paperless-ngx correcta');
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            probandoPaperless = false;
        }
    }
</script>

<div class="space-y-4">
    <div>
        <h2 class="text-lg font-semibold text-slate-800">Configuración de integraciones</h2>
        <p class="text-sm text-slate-500">
            Conexiones que utiliza el servidor para consultar expedientes y traer documentos desde
            Paperless-ngx. Los tokens se guardan en el servidor y no se vuelven a mostrar.
        </p>
    </div>

    {#if cargando}
        <p class="text-sm text-slate-500">Cargando configuración…</p>
    {:else}
        <section class="tarjeta space-y-4 p-4">
            <div>
                <h3 class="text-sm font-semibold text-slate-700">API de consulta de expedientes</h3>
                <p class="mt-1 text-sm text-slate-500">
                    Al capturar el número de expediente en un DARFE, el botón “Consultar expediente”
                    llenará nombre, correo y teléfono. La CURP se llena si la API la devuelve.
                </p>
            </div>

            <div class="grid gap-3 md:grid-cols-2">
                <label class="block text-sm md:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">URL del servicio</span>
                    <input
                        class="campo"
                        type="url"
                        bind:value={urlExpediente}
                        placeholder="https://servidor.institucional/api/BuscaExpediente"
                    />
                    <span class="mt-1 block text-xs text-slate-500">
                        URL completa del endpoint; se envía un POST con <code>expediente</code> en JSON.
                    </span>
                </label>

                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Token Bearer</span>
                    <input
                        class="campo"
                        type="password"
                        autocomplete="off"
                        bind:value={tokenExpediente}
                        placeholder={tokenExpedienteConfigurado
                            ? 'Token configurado; deje vacío para conservarlo'
                            : 'Token de acceso'}
                    />
                </label>

                <label class="flex items-end gap-2 pb-2 text-sm text-slate-700">
                    <input type="checkbox" bind:checked={activoExpediente} />
                    Integración activa
                </label>
            </div>

            <div class="flex flex-wrap items-center gap-2">
                <button
                    class="btn-primario"
                    type="button"
                    onclick={() => guardar('EXPEDIENTE')}
                    disabled={guardando === 'EXPEDIENTE'}
                >
                    {guardando === 'EXPEDIENTE' ? 'Guardando…' : 'Guardar'}
                </button>
                {#if tokenExpedienteConfigurado}
                    <span class="insignia bg-emerald-100 text-emerald-700">Token configurado</span>
                {/if}
            </div>

            <div class="grid gap-3 border-t border-slate-100 pt-4 md:grid-cols-[1fr_auto]">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Expediente de prueba</span>
                    <input
                        class="campo"
                        bind:value={pruebaExpediente}
                        placeholder="00002898/2026"
                    />
                </label>
                <button
                    class="btn-secundario self-end"
                    type="button"
                    onclick={probarExpediente}
                    disabled={probandoExpediente}
                >
                    {probandoExpediente ? 'Consultando…' : 'Probar consulta'}
                </button>
            </div>

            {#if resultadoExpediente}
                <div class="rounded-lg bg-slate-50 p-3 text-sm text-slate-700">
                    <p><strong>Nombre:</strong> {resultadoExpediente.nombre || '—'}</p>
                    <p><strong>Correo:</strong> {resultadoExpediente.correo || '—'}</p>
                    <p><strong>Teléfono:</strong> {resultadoExpediente.telefono || '—'}</p>
                    <p>
                        <strong>CURP:</strong>
                        {resultadoExpediente.curp || 'La API no devolvió CURP'}
                    </p>
                </div>
            {/if}
        </section>

        <section class="tarjeta space-y-4 p-4">
            <div>
                <h3 class="text-sm font-semibold text-slate-700">Paperless-ngx</h3>
                <p class="mt-1 text-sm text-slate-500">
                    Permite importar un documento por su ID desde Paperless hacia un campo de
                    documentos del DARFE. Use la URL base del servidor, sin <code>/api</code>.
                </p>
            </div>

            <div class="grid gap-3 md:grid-cols-2">
                <label class="block text-sm md:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">URL base</span>
                    <input
                        class="campo"
                        type="url"
                        bind:value={urlPaperless}
                        placeholder="https://paperless.institucional.mx"
                    />
                </label>

                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Token de Paperless</span>
                    <input
                        class="campo"
                        type="password"
                        autocomplete="off"
                        bind:value={tokenPaperless}
                        placeholder={tokenPaperlessConfigurado
                            ? 'Token configurado; deje vacío para conservarlo'
                            : 'Token de API de Paperless'}
                    />
                </label>

                <label class="flex items-end gap-2 pb-2 text-sm text-slate-700">
                    <input type="checkbox" bind:checked={activoPaperless} />
                    Integración activa
                </label>
            </div>

            <div class="flex flex-wrap items-center gap-2">
                <button
                    class="btn-primario"
                    type="button"
                    onclick={() => guardar('PAPERLESS')}
                    disabled={guardando === 'PAPERLESS'}
                >
                    {guardando === 'PAPERLESS' ? 'Guardando…' : 'Guardar'}
                </button>
                <button
                    class="btn-secundario"
                    type="button"
                    onclick={probarPaperless}
                    disabled={probandoPaperless || !activoPaperless}
                >
                    {probandoPaperless ? 'Probando…' : 'Probar conexión'}
                </button>
                {#if tokenPaperlessConfigurado}
                    <span class="insignia bg-emerald-100 text-emerald-700">Token configurado</span>
                {/if}
            </div>
        </section>
    {/if}
</div>
