<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api, type Expediente } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { POR_PAGINA, recortar, totalPaginas } from '../paginacion';

    let buscar = $state('');
    let expedientes = $state<Expediente[]>([]);
    let seleccionado = $state<any>(null);
    let historial = $state<any[]>([]);
    let local = $state(false);
    let pagina = $state(1);

    const paginas = $derived(totalPaginas(expedientes.length, POR_PAGINA));
    const visibles = $derived(recortar(expedientes, pagina, POR_PAGINA));

    let creando = $state(false);
    let numeroNuevo = $state('');
    let descripcionNueva = $state('');
    let unidadNueva = $state('');
    let guardando = $state(false);
    let anioNuevo = $state<number | null>(new Date().getFullYear());

    let importando = $state(false);
    let textoImportacion = $state('');
    let archivoNombre = $state('');
    let procesandoImportacion = $state(false);
    let resultadoImportacion = $state<any>(null);

    function abrirImportacion() {
        textoImportacion = '';
        archivoNombre = '';
        resultadoImportacion = null;
        importando = true;
    }

    function parsearFilas(texto: string) {
        return texto
            .split(/\r?\n/)
            .map((linea) => linea.trim())
            .filter((linea) => linea.length > 0 && !linea.startsWith('#'))
            .map((linea) => {
                const partes = (linea.includes(';') ? linea.split(';') : linea.split(',')).map((parte) =>
                    parte.trim()
                );
                const [numero, unidad, anio, ...resto] = partes;
                const anioNumero = anio ? Number(anio) : null;
                return {
                    numero: numero ?? '',
                    unidad: unidad || null,
                    anio: anioNumero && Number.isFinite(anioNumero) ? anioNumero : null,
                    descripcion: resto.join(', ').trim() || null
                };
            });
    }

    async function leerArchivo(evento: Event) {
        const entrada = evento.currentTarget as HTMLInputElement;
        const archivo = entrada.files?.[0];
        if (!archivo) return;
        archivoNombre = archivo.name;
        textoImportacion = await archivo.text();
    }

    async function importarLista() {
        const filas = parsearFilas(textoImportacion);
        if (filas.length === 0) {
            app.avisar('No hay filas para importar', 'error');
            return;
        }
        if (filas.length > 500) {
            app.avisar('El máximo por lote es 500 expedientes', 'error');
            return;
        }

        procesandoImportacion = true;
        resultadoImportacion = null;
        try {
            resultadoImportacion = await api('POST', '/api/v1/expedientes/importar', {
                expedientes: filas
            });
            app.avisar(
                `Importados ${resultadoImportacion.importados}; omitidos ${resultadoImportacion.omitidos}`
            );
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            procesandoImportacion = false;
        }
    }

    function abrirAlta() {
        numeroNuevo = '';
        descripcionNueva = '';
        unidadNueva = '';
        anioNuevo = new Date().getFullYear();
        creando = true;
    }

    let temporizador: ReturnType<typeof setTimeout>;

    onMount(cargar);

    function conRetardo() {
        clearTimeout(temporizador);
        temporizador = setTimeout(cargar, 250);
    }

    async function cargar() {
        try {
            const respuesta = await api<{ expedientes: Expediente[] }>(
                'GET',
                `/api/v1/expedientes?buscar=${encodeURIComponent(buscar)}`
            );
            expedientes = respuesta.expedientes;
            local = false;
            pagina = 1;
        } catch {
            // Sin conexión: se consulta la proyección local autorizada previamente sincronizada.
            try {
                const cache = await invoke<any[]>('listar_expedientes', { filtro: buscar });
                expedientes = cache.map((item) => ({
                    ...item,
                    custodio: item.custodio_id ? 'En custodia' : null
                }));
                pagina = 1;
                local = true;
            } catch (error) {
                app.avisar(String(error), 'error');
            }
        }
    }

    async function crearExpediente() {
        guardando = true;
        try {
            await api('POST', '/api/v1/expedientes', {
                numero: numeroNuevo,
                descripcion: descripcionNueva || null,
                unidad: unidadNueva || null,
                anio: anioNuevo || null
            });
            app.avisar('Expediente registrado; queda bajo tu custodia');
            creando = false;
            numeroNuevo = '';
            descripcionNueva = '';
            unidadNueva = '';
            anioNuevo = new Date().getFullYear();
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            guardando = false;
        }
    }

    async function abrir(expediente: Expediente) {
        try {
            const respuesta = await api<any>('GET', `/api/v1/expedientes/${expediente.id}/historial`);
            seleccionado = respuesta.expediente;
            historial = respuesta.movimientos;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-center gap-3">
        <input
            class="campo max-w-md"
            placeholder="Buscar por número o descripción…"
            bind:value={buscar}
            oninput={conRetardo}
        />
        <button class="btn-secundario" onclick={cargar}>Actualizar</button>
        {#if app.permiso('expedientes:crear')}
            <button class="btn-primario" onclick={abrirAlta}>Nuevo expediente</button>
        {/if}
        {#if app.permiso('expedientes:administrar')}
            <button class="btn-secundario" onclick={abrirImportacion}>Importar lista</button>
        {/if}
        <span class="text-sm text-slate-500">{expedientes.length} resultados</span>
    </div>

    {#if local}
        <p class="rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-700">
            Sin conexión: mostrando la copia local de expedientes. La información puede estar
            desactualizada.
        </p>
    {/if}

    <div class="tarjeta overflow-hidden">
        <div class="max-h-[65vh] overflow-auto">
            <table class="tabla">
                <thead>
                    <tr>
                        <th>Número</th>
                        <th>Descripción</th>
                        <th>Estado</th>
                        <th>Custodio</th>
                        <th>Versión</th>
                    </tr>
                </thead>
                <tbody>
                    {#each visibles as expediente}
                        <tr
                            class="cursor-pointer"
                            onclick={() => abrir(expediente)}
                            onkeydown={(e) => e.key === 'Enter' && abrir(expediente)}
                            tabindex="0"
                        >
                            <td class="font-medium text-institucional-600">{expediente.numero}</td>
                            <td class="text-slate-600">{expediente.descripcion}</td>
                            <td><Insignia estado={expediente.estado} /></td>
                            <td class="text-slate-500">{expediente.custodio ?? '—'}</td>
                            <td class="text-slate-400">{expediente.version}</td>
                        </tr>
                    {/each}
                    {#if expedientes.length === 0}
                        <tr><td colspan="5" class="py-6 text-center text-slate-400">Sin resultados</td></tr>
                    {/if}
                </tbody>
            </table>
        </div>
        <Paginacion
            {pagina}
            {paginas}
            total={expedientes.length}
            onCambiar={(valor) => (pagina = valor)}
        />
    </div>
</div>

{#if importando}
    <Modal titulo="Importar expedientes" oncerrar={() => (importando = false)}>
        <div class="space-y-4">
            <p class="rounded-lg bg-sky-50 px-3 py-2 text-xs text-sky-800">
                Una fila por expediente: <code>numero;unidad;año;descripción</code>. Solo el número es
                obligatorio; se completa a 8 caracteres antes de la barra y los duplicados se omiten.
                Los expedientes importados quedan en <strong>almacén</strong>, sin custodia.
            </p>

            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Archivo CSV o TXT (opcional)</span>
                <input class="campo" type="file" accept=".csv,.txt" onchange={leerArchivo} />
                {#if archivoNombre}
                    <span class="mt-1 block text-xs text-slate-500">{archivoNombre}</span>
                {/if}
            </label>

            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">O pegue la lista</span>
                <textarea
                    class="campo h-40 font-mono text-xs"
                    bind:value={textoImportacion}
                    placeholder={'5259/2026;JUZGADO 1;2026;Demanda laboral\n100/2026'}
                ></textarea>
            </label>

            <button class="btn-primario" disabled={procesandoImportacion} onclick={importarLista}>
                {procesandoImportacion ? 'Importando…' : 'Importar'}
            </button>

            {#if resultadoImportacion}
                <div class="rounded-lg border border-slate-200 p-3 text-sm">
                    <p>
                        <strong>{resultadoImportacion.importados}</strong> importados ·
                        <strong>{resultadoImportacion.omitidos}</strong> omitidos (duplicados) ·
                        <strong>{resultadoImportacion.errores.length}</strong> con error
                    </p>
                    {#if resultadoImportacion.errores.length > 0}
                        <ul class="mt-2 list-inside list-disc text-xs text-rose-700">
                            {#each resultadoImportacion.errores.slice(0, 10) as error}
                                <li>Fila {error.fila}: {error.motivo}</li>
                            {/each}
                        </ul>
                    {/if}
                </div>
            {/if}
        </div>
    </Modal>
{/if}

{#if creando}
    <Modal titulo="Nuevo expediente" oncerrar={() => (creando = false)}>
        <form
            class="space-y-4"
            onsubmit={(evento) => {
                evento.preventDefault();
                crearExpediente();
            }}
        >
            <div class="grid gap-3 sm:grid-cols-2">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Número de expediente</span>
                    <input class="campo" bind:value={numeroNuevo} required />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Unidad / órgano</span>
                    <input class="campo" bind:value={unidadNueva} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Año</span>
                    <input class="campo" type="number" min="1900" max="2200" bind:value={anioNuevo} />
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Descripción</span>
                    <input class="campo" bind:value={descripcionNueva} />
                </label>
            </div>

            <p class="text-xs text-slate-500">
                El número se completa con ceros a la izquierda hasta 8 caracteres antes de la barra
                (5259/2026 → 00005259/2026). El número es único en todo el sistema. Al registrarlo, el expediente
                queda bajo tu custodia hasta que lo devuelvas al almacén.
            </p>

            <button class="btn-primario" type="submit" disabled={guardando}>
                {guardando ? 'Guardando…' : 'Registrar expediente'}
            </button>
        </form>
    </Modal>
{/if}

{#if seleccionado}
    <Modal titulo={`Expediente ${seleccionado.numero}`} oncerrar={() => (seleccionado = null)}>
        <dl class="mb-4 grid grid-cols-2 gap-3 text-sm">
            <div><dt class="text-slate-500">Estado</dt><dd><Insignia estado={seleccionado.estado} /></dd></div>
            <div><dt class="text-slate-500">Custodio actual</dt><dd>{seleccionado.custodio ?? 'Almacén'}</dd></div>
        </dl>
        <h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
            Historial de movimientos
        </h3>
        <ul class="space-y-2">
            {#each historial as movimiento}
                <li class="rounded-lg border border-slate-100 px-3 py-2 text-sm">
                    <div class="flex justify-between">
                        <span class="font-medium text-slate-700">{movimiento.tipo.replaceAll('_', ' ')}</span>
                        <span class="text-xs text-slate-400">
                            {new Date(movimiento.creado_en).toLocaleString('es-MX')}
                        </span>
                    </div>
                    <p class="text-xs text-slate-500">
                        {movimiento.actor}{movimiento.detalle ? ` · ${movimiento.detalle}` : ''}
                    </p>
                </li>
            {/each}
            {#if historial.length === 0}
                <li class="text-sm text-slate-400">Sin movimientos registrados</li>
            {/if}
        </ul>
    </Modal>
{/if}
