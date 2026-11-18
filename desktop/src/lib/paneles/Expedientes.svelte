<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api, type Expediente } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { totalPaginas } from '../paginacion';

    const TAMANO = 25;
    const ANIO_INICIAL = 2020;

    let buscar = $state('');
    let expedientes = $state<Expediente[]>([]);
    let total = $state(0);
    let porEstado = $state<Record<string, number>>({});
    let unidades = $state<string[]>([]);
    let anios = $state<number[]>([]);
    let pagina = $state(1);
    let local = $state(false);
    let avanzado = $state(false);

    let estadoFiltro = $state('');
    let unidadFiltro = $state('');
    let anioFiltro = $state('');
    let custodioFiltro = $state('');

    let seleccionado = $state<any>(null);
    let historial = $state<any[]>([]);

    let creando = $state(false);
    let numeroNuevo = $state('');
    let descripcionNueva = $state('');
    let unidadNueva = $state('');
    let anioActual = $state(new Date().getFullYear());
    let anioNuevo = $state(String(new Date().getFullYear()));
    const aniosDisponibles = $derived(
        Array.from({ length: Math.max(0, anioActual - ANIO_INICIAL + 1) }, (_, indice) =>
            String(anioActual - indice)
        )
    );
    let guardando = $state(false);

    let importando = $state(false);
    let textoImportacion = $state('');
    let archivoNombre = $state('');
    let procesandoImportacion = $state(false);
    let resultadoImportacion = $state<any>(null);

    const paginas = $derived(totalPaginas(total, TAMANO));

    let temporizador: ReturnType<typeof setTimeout>;

    onMount(cargar);

    function conRetardo() {
        clearTimeout(temporizador);
        temporizador = setTimeout(() => {
            pagina = 1;
            cargar();
        }, 250);
    }

    async function cargar() {
        const parametros = new URLSearchParams();
        if (buscar.trim()) parametros.set('buscar', buscar.trim());
        if (estadoFiltro) parametros.set('estado', estadoFiltro);
        if (unidadFiltro) parametros.set('unidad', unidadFiltro);
        if (anioFiltro) parametros.set('anio', anioFiltro);
        if (custodioFiltro.trim()) parametros.set('custodio', custodioFiltro.trim());
        parametros.set('pagina', String(pagina));
        parametros.set('tamano', String(TAMANO));

        try {
            const respuesta = await api<any>('GET', `/api/v1/expedientes?${parametros.toString()}`);
            expedientes = respuesta.expedientes;
            total = respuesta.total;
            porEstado = respuesta.por_estado ?? {};
            unidades = respuesta.unidades ?? [];
            anios = respuesta.anios ?? [];
            local = false;
        } catch {
            // Sin conexión: se consulta la proyección local autorizada.
            try {
                const cache = await invoke<any[]>('listar_expedientes', { filtro: buscar });
                expedientes = cache.map((item) => ({
                    ...item,
                    custodio: item.custodio_id ? 'En custodia' : null
                }));
                total = cache.length;
                porEstado = {};
                local = true;
            } catch (error) {
                app.avisar(String(error), 'error');
            }
        }
    }

    function aplicarFiltros() {
        pagina = 1;
        cargar();
    }

    function limpiarFiltros() {
        estadoFiltro = '';
        unidadFiltro = '';
        anioFiltro = '';
        custodioFiltro = '';
        pagina = 1;
        cargar();
    }

    function alternarEstado(estado: string) {
        estadoFiltro = estadoFiltro === estado ? '' : estado;
        aplicarFiltros();
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

    function abrirAlta() {
        numeroNuevo = '';
        descripcionNueva = '';
        unidadNueva = '';
        anioActual = new Date().getFullYear();
        anioNuevo = String(anioActual);
        creando = true;
    }

    async function crearExpediente() {
        guardando = true;
        try {
            await api('POST', '/api/v1/expedientes', {
                numero: numeroNuevo,
                descripcion: descripcionNueva || null,
                unidad: unidadNueva || null,
                anio: anioNuevo ? Number(anioNuevo) : null
            });
            app.avisar('Expediente registrado; queda bajo tu custodia');
            creando = false;
            pagina = 1;
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            guardando = false;
        }
    }

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
                const [numero, ...resto] = partes;
                return {
                    numero: numero ?? '',
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
            pagina = 1;
            await cargar();
            await app.sincronizar(true);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            procesandoImportacion = false;
        }
    }
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
            <p class="text-2xl font-semibold text-slate-800">
                {total.toLocaleString('es-MX')}
                <span class="text-base font-normal text-slate-500">expedientes</span>
            </p>
            <p class="text-xs text-slate-500">
                {#if local}
                    Copia local · la información puede estar desactualizada
                {:else}
                    Inventario institucional
                {/if}
            </p>
        </div>
        <div class="flex flex-wrap gap-2">
            {#if app.permiso('expedientes:crear')}
                <button class="btn-primario" onclick={abrirAlta}>Nuevo expediente</button>
            {/if}
            {#if app.permiso('expedientes:administrar')}
                <button class="btn-secundario" onclick={abrirImportacion}>Importar lista</button>
            {/if}
            <button class="btn-secundario" onclick={cargar}>Actualizar</button>
        </div>
    </div>

    <div class="flex flex-wrap items-center gap-3">
        <input
            class="campo max-w-md"
            placeholder="Buscar por número o descripción…"
            bind:value={buscar}
            oninput={conRetardo}
        />
        <button class="btn-secundario" onclick={() => (avanzado = !avanzado)}>
            {avanzado ? 'Ocultar filtros' : 'Filtros avanzados'}
        </button>
        {#if estadoFiltro || unidadFiltro || anioFiltro || custodioFiltro}
            <button class="text-xs text-institucional-600 underline" onclick={limpiarFiltros}>
                Limpiar filtros
            </button>
        {/if}
    </div>

    {#if Object.keys(porEstado).length > 0}
        <div class="flex flex-wrap gap-2">
            {#each Object.entries(porEstado) as [estado, cantidad]}
                <button
                    class="flex items-center gap-2 rounded-full border px-3 py-1 text-xs {estadoFiltro === estado
                        ? 'border-institucional-500 bg-institucional-50'
                        : 'border-slate-200 bg-white hover:bg-slate-50'}"
                    onclick={() => alternarEstado(estado)}
                >
                    <Insignia {estado} />
                    <span class="font-semibold text-slate-700">{cantidad}</span>
                </button>
            {/each}
        </div>
    {/if}

    {#if avanzado}
        <div class="tarjeta grid gap-3 p-4 sm:grid-cols-2 lg:grid-cols-4">
            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Estado</span>
                <select class="campo" bind:value={estadoFiltro} onchange={aplicarFiltros}>
                    <option value="">Todos</option>
                    <option value="ALMACEN">Almacén</option>
                    <option value="EN_CUSTODIA">En custodia</option>
                    <option value="NO_LOCALIZADO">No localizado</option>
                </select>
            </label>

            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Unidad / órgano</span>
                <select class="campo" bind:value={unidadFiltro} onchange={aplicarFiltros}>
                    <option value="">Todas</option>
                    {#each unidades as unidad}
                        <option value={unidad}>{unidad}</option>
                    {/each}
                </select>
            </label>

            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Año</span>
                <select class="campo" bind:value={anioFiltro} onchange={aplicarFiltros}>
                    <option value="">Todos</option>
                    {#each anios as anio}
                        <option value={String(anio)}>{anio}</option>
                    {/each}
                </select>
            </label>

            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Custodio</span>
                <input
                    class="campo"
                    placeholder="Nombre del custodio"
                    bind:value={custodioFiltro}
                    onchange={aplicarFiltros}
                    onkeydown={(evento) => evento.key === 'Enter' && aplicarFiltros()}
                />
            </label>
        </div>
    {/if}

    {#if local}
        <p class="rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-700">
            Sin conexión: mostrando la copia local de expedientes. Los filtros avanzados requieren
            conexión con el servidor.
        </p>
    {/if}

    <div class="tarjeta overflow-hidden">
        <div class="max-h-[62vh] overflow-auto">
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
                    {#each expedientes as expediente}
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
            {total}
            onCambiar={(valor) => {
                pagina = valor;
                cargar();
            }}
        />
    </div>
</div>

{#if importando}
    <Modal titulo="Importar expedientes" oncerrar={() => (importando = false)}>
        <div class="space-y-4">
            <p class="rounded-lg bg-sky-50 px-3 py-2 text-xs text-sky-800">
                Una fila por expediente: <code>numero;descripción</code>. El número se completa a 8
                caracteres antes de la barra y los duplicados se omiten. Los expedientes importados se
                asignan al <strong>almacén</strong> (sin custodia) y usan el año en curso.
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
                    placeholder={'5259/2026;Demanda laboral\n100/2026'}
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
                    <select class="campo" bind:value={anioNuevo}>
                        {#each aniosDisponibles as anio}
                            <option value={anio}>{anio}</option>
                        {/each}
                    </select>
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Descripción</span>
                    <input class="campo" bind:value={descripcionNueva} />
                </label>
            </div>

            <p class="text-xs text-slate-500">
                El número se completa con ceros a la izquierda hasta 8 caracteres antes de la barra
                (5259/2026 → 00005259/2026). El número es único en todo el sistema. Al registrarlo, el
                expediente queda bajo tu custodia hasta que lo devuelvas al almacén.
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
