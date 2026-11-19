<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { totalPaginas } from '../paginacion';

    const TAMANO = 25;
    const ESTATUS = ['PENDIENTE', 'INCOMPLETO', 'COMPLETO'];
    const CAMPOS_DOCUMENTO = [
        { clave: 'CURP', etiqueta: 'CURP' },
        { clave: 'RFC', etiqueta: 'RFC' },
        { clave: 'REGISTRO', etiqueta: 'Registro' },
        { clave: 'IDENTIFICACION', etiqueta: 'Identificación' },
        { clave: 'ESTADO_CUENTA', etiqueta: 'Estado de cuenta' },
        { clave: 'DEPOSITO', etiqueta: 'Depósito' }
    ];

    type CampoExtra = { nombre: string; valor: string };
    type Archivos = Record<string, File | null>;

    let registros = $state<any[]>([]);
    let total = $state(0);
    let pagina = $state(1);
    let pestana = $state<'registros' | 'reportes'>('registros');
    let avanzado = $state(false);
    let cargando = $state(false);
    let filtros = $state({
        buscar: '',
        fecha_desde: '',
        fecha_hasta: '',
        estatus: '',
        banco: '',
        tipo: ''
    });
    let reportes = $state<any>(null);
    let modal = $state(false);
    let editandoId = $state<string | null>(null);
    let guardando = $state(false);
    let errorFormulario = $state('');
    let documentosExistentes = $state<string[]>([]);
    let archivos = $state<Archivos>(vacioArchivos());
    let form = $state({
        numero_darfe: '',
        fecha: fechaHoy(),
        nombre: '',
        expediente: '',
        registro: '',
        estado_cuenta: '',
        banco: '',
        identificacion: '',
        curp: '',
        rfc: '',
        descripcion: '',
        monto: '',
        numero_cuenta: '',
        tipo: 'REEMBOLSO',
        estatus: 'PENDIENTE',
        campos_extra: [] as CampoExtra[]
    });

    const paginas = $derived(totalPaginas(total, TAMANO));

    onMount(cargar);

    function fechaHoy(): string {
        const hoy = new Date();
        return `${hoy.getFullYear()}-${String(hoy.getMonth() + 1).padStart(2, '0')}-${String(
            hoy.getDate()
        ).padStart(2, '0')}`;
    }

    function vacioArchivos(): Archivos {
        return Object.fromEntries(CAMPOS_DOCUMENTO.map((campo) => [campo.clave, null]));
    }

    function parametros(): string {
        const params = new URLSearchParams();
        for (const [clave, valor] of Object.entries(filtros)) {
            if (String(valor).trim()) params.set(clave, String(valor).trim());
        }
        params.set('pagina', String(pagina));
        params.set('tamano', String(TAMANO));
        return params.toString();
    }

    async function cargar() {
        if (pestana === 'reportes') {
            await cargarReportes();
            return;
        }
        cargando = true;
        try {
            const respuesta = await api<{ darfes: any[]; total: number }>(
                'GET',
                `/api/v1/darfes?${parametros()}`
            );
            registros = respuesta.darfes;
            total = respuesta.total;
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        } finally {
            cargando = false;
        }
    }

    async function cargarReportes() {
        try {
            reportes = await api('GET', '/api/v1/darfes/reportes');
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    function buscar() {
        pagina = 1;
        cargar();
    }

    function limpiarFiltros() {
        filtros = { buscar: '', fecha_desde: '', fecha_hasta: '', estatus: '', banco: '', tipo: '' };
        pagina = 1;
        cargar();
    }

    function abrirNuevo() {
        editandoId = null;
        documentosExistentes = [];
        archivos = vacioArchivos();
        errorFormulario = '';
        form = {
            numero_darfe: '',
            fecha: fechaHoy(),
            nombre: '',
            expediente: '',
            registro: '',
            estado_cuenta: '',
            banco: '',
            identificacion: '',
            curp: '',
            rfc: '',
            descripcion: '',
            monto: '',
            numero_cuenta: '',
            tipo: 'REEMBOLSO',
            estatus: 'PENDIENTE',
            campos_extra: []
        };
        modal = true;
    }

    function abrirEdicion(registro: any) {
        editandoId = registro.id;
        documentosExistentes = Array.isArray(registro.documentos) ? registro.documentos : [];
        archivos = vacioArchivos();
        errorFormulario = '';
        form = {
            numero_darfe: registro.numero_darfe,
            fecha: registro.fecha,
            nombre: registro.nombre,
            expediente: registro.expediente ?? '',
            registro: registro.registro ?? '',
            estado_cuenta: registro.estado_cuenta ?? '',
            banco: registro.banco ?? '',
            identificacion: registro.identificacion ?? '',
            curp: registro.curp ?? '',
            rfc: registro.rfc ?? '',
            descripcion: registro.descripcion ?? '',
            monto: String(registro.monto ?? ''),
            numero_cuenta: registro.numero_cuenta ?? '',
            tipo: registro.tipo ?? 'REEMBOLSO',
            estatus: registro.estatus,
            campos_extra: Array.isArray(registro.campos_extra)
                ? registro.campos_extra.map((campo: any) => ({ nombre: campo.nombre, valor: campo.valor }))
                : []
        };
        modal = true;
    }

    function agregarCampo() {
        form.campos_extra = [...form.campos_extra, { nombre: '', valor: '' }];
    }

    function quitarCampo(indice: number) {
        form.campos_extra = form.campos_extra.filter((_, i) => i !== indice);
    }

    function seleccionarArchivo(evento: Event, campo: string) {
        const entrada = evento.currentTarget as HTMLInputElement;
        archivos = { ...archivos, [campo]: entrada.files?.[0] ?? null };
    }

    function soltarArchivo(evento: DragEvent, campo: string) {
        evento.preventDefault();
        const archivo = evento.dataTransfer?.files?.[0];
        if (archivo) archivos = { ...archivos, [campo]: archivo };
    }

    function leerBase64(archivo: File): Promise<string> {
        return new Promise((resolve, reject) => {
            const lector = new FileReader();
            lector.onload = () => resolve(String(lector.result).split(',')[1] ?? '');
            lector.onerror = () => reject(lector.error);
            lector.readAsDataURL(archivo);
        });
    }

    async function guardar() {
        guardando = true;
        errorFormulario = '';
        try {
            const cuerpo = {
                numero_darfe: form.numero_darfe,
                fecha: form.fecha,
                nombre: form.nombre,
                expediente: form.expediente,
                registro: form.registro,
                estado_cuenta: form.estado_cuenta,
                banco: form.banco,
                identificacion: form.identificacion,
                curp: form.curp,
                rfc: form.rfc,
                descripcion: form.descripcion,
                monto: Number(form.monto),
                numero_cuenta: form.numero_cuenta,
                tipo: form.tipo,
                estatus: form.estatus,
                campos_extra: form.campos_extra.filter(
                    (campo) => campo.nombre.trim() !== '' || campo.valor.trim() !== ''
                )
            };
            let ids: string[] = [];
            if (editandoId) {
                const respuesta = await api<any>('PUT', `/api/v1/darfes/${editandoId}`, cuerpo);
                ids = [respuesta.id];
            } else {
                const respuesta = await api<any>('POST', '/api/v1/darfes', cuerpo);
                ids = Array.isArray(respuesta.darfes)
                    ? respuesta.darfes.map((item: any) => item.id)
                    : [respuesta.id];
            }

            let depositoCoincide: boolean | null = null;
            for (const id of ids) {
                for (const campo of CAMPOS_DOCUMENTO) {
                    const archivo = archivos[campo.clave];
                    if (!archivo) continue;
                    const resultado = await api<any>('POST', `/api/v1/darfes/${id}/documentos`, {
                        campo: campo.clave,
                        nombre: archivo.name,
                        mime: archivo.type || 'application/octet-stream',
                        contenido_base64: await leerBase64(archivo)
                    });
                    if (campo.clave === 'DEPOSITO') depositoCoincide = resultado.coincide;
                }
            }

            if (ids.length > 1) {
                app.avisar(`${ids.length} canastas creadas`);
            } else {
                app.avisar(editandoId ? 'DARFE actualizado' : 'DARFE creado');
            }
            if (depositoCoincide === true) {
                app.avisar('Depósito verificado: los datos coinciden');
            } else if (depositoCoincide === false) {
                app.avisar('Depósito cargado, pero los datos no coinciden', 'error');
            }
            modal = false;
            pagina = 1;
            await cargar();
        } catch (fallo) {
            errorFormulario = String(fallo);
        } finally {
            guardando = false;
        }
    }

    async function eliminar(registro: any) {
        if (!confirm(`¿Eliminar el DARFE ${registro.numero_darfe}?`)) return;
        try {
            await api('DELETE', `/api/v1/darfes/${registro.id}`);
            app.avisar('DARFE eliminado');
            await cargar();
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    async function descargarDocumento(registro: any, campo: string) {
        try {
            const ruta = await invoke<string>('guardar_documento_darfe', {
                id: registro.id,
                campo
            });
            app.avisar(`Documento guardado en ${ruta}`);
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    async function exportar() {
        try {
            const consulta = new URLSearchParams();
            for (const [clave, valor] of Object.entries(filtros)) {
                if (String(valor).trim()) consulta.set(clave, String(valor).trim());
            }
            const ruta = await invoke<string>('guardar_exportacion_darfes', {
                consulta: consulta.toString() ? `?${consulta.toString()}` : ''
            });
            app.avisar(`Excel guardado en ${ruta}`);
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    function fechaVisible(fecha: string): string {
        return new Date(`${fecha}T00:00:00`).toLocaleDateString('es-MX');
    }

    function pares(valor: any): Array<{ clave: string; total: number }> {
        if (!valor || typeof valor !== 'object') return [];
        return Object.entries(valor).map(([clave, total]) => ({ clave, total: Number(total) }));
    }
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
            <h2 class="text-lg font-semibold text-slate-800">DARFES</h2>
            <p class="text-sm text-slate-500">Registro, documentos, exportación y reportes.</p>
        </div>
        <div class="flex flex-wrap gap-2">
            <button
                class="btn-secundario"
                type="button"
                onclick={() => {
                    pestana = 'registros';
                    cargar();
                }}
            >
                Registros
            </button>
            <button
                class="btn-secundario"
                type="button"
                onclick={() => {
                    pestana = 'reportes';
                    cargarReportes();
                }}
            >
                Reportes e indicadores
            </button>
            {#if pestana === 'registros' && app.permiso('darfes:crear')}
                <button class="btn-primario" type="button" onclick={abrirNuevo}>Nuevo DARFE</button>
            {/if}
        </div>
    </div>

    {#if pestana === 'registros'}
        <section class="tarjeta space-y-3 p-4">
            <div class="flex flex-wrap gap-2">
                <input
                    class="campo min-w-64 flex-1"
                    placeholder="Buscar por número, nombre, expediente, CURP, RFC, banco…"
                    bind:value={filtros.buscar}
                    onkeydown={(evento) => evento.key === 'Enter' && buscar()}
                />
                <button class="btn-secundario" type="button" onclick={buscar} disabled={cargando}>
                    {cargando ? 'Buscando…' : 'Buscar'}
                </button>
                <button class="btn-secundario" type="button" onclick={() => (avanzado = !avanzado)}>
                    {avanzado ? 'Ocultar filtros' : 'Filtros'}
                </button>
                {#if app.permiso('darfes:exportar')}
                    <button class="btn-secundario" type="button" onclick={exportar}>Exportar Excel</button>
                {/if}
            </div>

            {#if avanzado}
                <div class="grid gap-3 md:grid-cols-4">
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Fecha desde</span>
                        <input class="campo" type="date" bind:value={filtros.fecha_desde} />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Fecha hasta</span>
                        <input class="campo" type="date" bind:value={filtros.fecha_hasta} />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Estatus</span>
                        <select class="campo" bind:value={filtros.estatus}>
                            <option value="">Todos</option>
                            {#each ESTATUS as estatus}
                                <option value={estatus}>{estatus}</option>
                            {/each}
                        </select>
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Banco</span>
                        <input class="campo" bind:value={filtros.banco} />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Tipo</span>
                        <select class="campo" bind:value={filtros.tipo}>
                            <option value="">Todos</option>
                            <option value="CANASTA">CANASTA</option>
                            <option value="REEMBOLSO">REEMBOLSO</option>
                        </select>
                    </label>
                    <div class="flex items-end gap-2 md:col-span-5">
                        <button class="btn-secundario" type="button" onclick={limpiarFiltros}>Limpiar</button>
                        <button class="btn-primario" type="button" onclick={buscar}>Aplicar</button>
                    </div>
                </div>
            {/if}
        </section>

        <section class="tarjeta overflow-hidden">
            <div class="max-h-[62vh] overflow-auto">
                <table class="tabla">
                    <thead>
                        <tr>
                            <th>No. DARFE</th>
                            <th>Fecha</th>
                            <th>Nombre</th>
                            <th>Descripción</th>
                            <th>Monto</th>
                            <th>Tipo</th>
                            <th>Banco</th>
                            <th>Estatus</th>
                            <th>Documentos</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each registros as registro (registro.id)}
                            <tr>
                                <td class="font-mono text-xs font-medium text-institucional-600">
                                    {registro.numero_darfe}
                                </td>
                                <td>{fechaVisible(registro.fecha)}</td>
                                <td>{registro.nombre}</td>
                                <td>{registro.descripcion || '—'}</td>
                                <td>${Number(registro.monto ?? 0).toLocaleString('es-MX', { minimumFractionDigits: 2 })}</td>
                                <td>{registro.tipo}</td>
                                <td>{registro.banco || '—'}</td>
                                <td>
                                    <span
                                        class="insignia {registro.estatus === 'COMPLETO'
                                            ? 'bg-emerald-100 text-emerald-700'
                                            : registro.estatus === 'INCOMPLETO'
                                              ? 'bg-amber-100 text-amber-700'
                                              : 'bg-slate-100 text-slate-600'}"
                                    >
                                        {registro.estatus}
                                    </span>
                                </td>
                                <td>
                                    <div class="flex flex-wrap gap-1">
                                        {#each CAMPOS_DOCUMENTO as campo}
                                            {#if registro.documentos?.includes(campo.clave)}
                                                <button
                                                    class="text-xs underline text-institucional-600"
                                                    type="button"
                                                    onclick={() => descargarDocumento(registro, campo.clave)}
                                                >
                                                    {campo.etiqueta}
                                                </button>
                                            {/if}
                                        {/each}
                                    </div>
                                </td>
                                <td class="space-x-2 whitespace-nowrap text-right">
                                    {#if app.permiso('darfes:editar')}
                                        <button class="btn-secundario" type="button" onclick={() => abrirEdicion(registro)}>
                                            Editar
                                        </button>
                                    {/if}
                                    {#if app.permiso('darfes:eliminar')}
                                        <button class="btn-secundario" type="button" onclick={() => eliminar(registro)}>
                                            Eliminar
                                        </button>
                                    {/if}
                                </td>
                            </tr>
                        {:else}
                            <tr>
                                <td colspan="10" class="py-8 text-center text-sm text-slate-500">
                                    No hay DARFES con esos filtros.
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
            <Paginacion
                {pagina}
                {paginas}
                total={total}
                onCambiar={(valor) => {
                    pagina = valor;
                    cargar();
                }}
            />
        </section>
    {:else}
        <section class="grid gap-4 sm:grid-cols-4">
            <div class="tarjeta p-4">
                <p class="text-xs uppercase tracking-wide text-slate-500">Total</p>
                <p class="mt-1 text-2xl font-semibold text-institucional-600">{reportes?.total ?? 0}</p>
            </div>
            {#each pares(reportes?.por_estatus) as item}
                <div class="tarjeta p-4">
                    <p class="text-xs uppercase tracking-wide text-slate-500">{item.clave}</p>
                    <p class="mt-1 text-2xl font-semibold text-institucional-600">{item.total}</p>
                </div>
            {/each}
        </section>

        <div class="grid gap-4 lg:grid-cols-2">
            <section class="tarjeta p-4">
                <h3 class="mb-3 text-sm font-semibold text-slate-700">Por banco</h3>
                {#each reportes?.por_banco ?? [] as item}
                    <div class="mb-2 flex items-center justify-between gap-3 text-sm">
                        <span class="truncate text-slate-600">{item.clave}</span>
                        <span class="font-semibold text-slate-800">{item.total}</span>
                    </div>
                {:else}
                    <p class="text-sm text-slate-500">Sin datos.</p>
                {/each}
            </section>

            <section class="tarjeta p-4">
                <h3 class="mb-3 text-sm font-semibold text-slate-700">Documentos cargados</h3>
                {#each pares(reportes?.documentos) as item}
                    <div class="mb-2 flex items-center justify-between gap-3 text-sm">
                        <span class="text-slate-600">{item.clave.replaceAll('_', ' ')}</span>
                        <span class="font-semibold text-slate-800">{item.total}</span>
                    </div>
                {:else}
                    <p class="text-sm text-slate-500">Sin documentos.</p>
                {/each}
            </section>
        </div>

        <section class="tarjeta p-4">
            <h3 class="mb-3 text-sm font-semibold text-slate-700">Últimos DARFES</h3>
            <ul class="space-y-2 text-sm">
                {#each reportes?.recientes ?? [] as item}
                    <li class="flex items-center justify-between gap-3 border-b border-slate-100 pb-2">
                        <span class="font-mono text-xs text-institucional-600">{item.numero_darfe}</span>
                        <span class="flex-1 truncate text-slate-600">{item.nombre}</span>
                        <span class="text-slate-500">{fechaVisible(item.fecha)}</span>
                        <span class="insignia bg-slate-100 text-slate-600">{item.estatus}</span>
                    </li>
                {:else}
                    <li class="text-slate-500">Sin registros.</li>
                {/each}
            </ul>
        </section>
    {/if}
</div>

{#if modal}
    <Modal titulo={editandoId ? 'Editar DARFE' : 'Nuevo DARFE'} oncerrar={() => (modal = false)}>
        <form
            class="space-y-4"
            onsubmit={(evento) => {
                evento.preventDefault();
                guardar();
            }}
        >
            <div class="grid gap-3 sm:grid-cols-2">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">No. de DARFE</span>
                    <input class="campo font-mono" maxlength="100" bind:value={form.numero_darfe} required />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Fecha</span>
                    <input class="campo" type="date" bind:value={form.fecha} required />
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Nombre</span>
                    <input class="campo" maxlength="200" bind:value={form.nombre} required />
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Descripción</span>
                    <textarea
                        class="campo h-20"
                        maxlength="500"
                        bind:value={form.descripcion}
                        disabled={form.tipo === 'CANASTA'}
                        placeholder={form.tipo === 'CANASTA'
                            ? 'Se generará automáticamente para cada canasta'
                            : 'Descripción del DARFE'}
                    ></textarea>
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Monto</span>
                    <input class="campo" type="number" min="0" step="0.01" bind:value={form.monto} required />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Número de cuenta</span>
                    <input class="campo" maxlength="50" bind:value={form.numero_cuenta} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Tipo</span>
                    <select class="campo" bind:value={form.tipo} disabled={Boolean(editandoId)}>
                        <option value="REEMBOLSO">REEMBOLSO</option>
                        <option value="CANASTA">CANASTA</option>
                    </select>
                    {#if form.tipo === 'CANASTA' && !editandoId}
                        <span class="mt-1 block text-xs text-slate-500">
                            Se crearán 3 registros mensuales automáticamente.
                        </span>
                    {/if}
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Expediente</span>
                    <input class="campo" bind:value={form.expediente} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Registro</span>
                    <input class="campo" bind:value={form.registro} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Estado de cuenta</span>
                    <input class="campo" bind:value={form.estado_cuenta} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Banco</span>
                    <input class="campo" bind:value={form.banco} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Identificación</span>
                    <input class="campo" bind:value={form.identificacion} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">CURP</span>
                    <input class="campo font-mono uppercase" bind:value={form.curp} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">RFC</span>
                    <input class="campo font-mono uppercase" bind:value={form.rfc} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Estatus</span>
                    <select class="campo" bind:value={form.estatus}>
                        {#each ESTATUS as estatus}
                            <option value={estatus}>{estatus}</option>
                        {/each}
                    </select>
                </label>
            </div>

            <div class="space-y-2">
                <div class="flex items-center justify-between">
                    <span class="text-sm font-medium text-slate-600">Documentos</span>
                    {#if editandoId}
                        <span class="text-xs text-slate-500">Los archivos nuevos reemplazan al existente.</span>
                    {/if}
                </div>
                <div class="grid gap-3 md:grid-cols-2">
                    {#each CAMPOS_DOCUMENTO as campo}
                        <div
                            class="rounded-lg border border-dashed border-slate-300 p-3 text-sm"
                            role="group"
                            ondragover={(evento) => evento.preventDefault()}
                            ondrop={(evento) => soltarArchivo(evento, campo.clave)}
                        >
                            <p class="font-medium text-slate-700">{campo.etiqueta}</p>
                            <p class="text-xs text-slate-500">Arrastra el archivo aquí o selecciónalo.</p>
                            {#if documentosExistentes.includes(campo.clave)}
                                <button
                                    class="mt-1 text-xs underline text-institucional-600"
                                    type="button"
                                    onclick={() => descargarDocumento({ id: editandoId }, campo.clave)}
                                >
                                    Descargar actual
                                </button>
                            {/if}
                            <input
                                class="campo mt-2 text-xs"
                                type="file"
                                accept="application/pdf,image/*"
                                onchange={(evento) => seleccionarArchivo(evento, campo.clave)}
                            />
                            {#if archivos[campo.clave]}
                                <p class="mt-1 truncate text-xs text-emerald-700">
                                    {archivos[campo.clave]?.name}
                                </p>
                            {/if}
                        </div>
                    {/each}
                </div>
            </div>

            <div class="space-y-2">
                <div class="flex items-center justify-between">
                    <span class="text-sm font-medium text-slate-600">Campos adicionales</span>
                    <button class="btn-secundario" type="button" onclick={agregarCampo}>Agregar campo</button>
                </div>
                {#each form.campos_extra as campo, indice}
                    <div class="flex gap-2">
                        <input class="campo" placeholder="Nombre del campo" bind:value={campo.nombre} />
                        <input class="campo" placeholder="Valor" bind:value={campo.valor} />
                        <button class="btn-secundario" type="button" onclick={() => quitarCampo(indice)}>
                            Quitar
                        </button>
                    </div>
                {/each}
            </div>

            {#if errorFormulario}
                <p class="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700">{errorFormulario}</p>
            {/if}

            <div class="flex justify-end gap-2">
                <button class="btn-secundario" type="button" onclick={() => (modal = false)} disabled={guardando}>
                    Cancelar
                </button>
                <button class="btn-primario" type="submit" disabled={guardando}>
                    {guardando ? 'Guardando…' : 'Guardar'}
                </button>
            </div>
        </form>
    </Modal>
{/if}
