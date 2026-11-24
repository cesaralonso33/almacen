<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import BotonIcono from '../BotonIcono.svelte';
    import Icono from '../Icono.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { totalPaginas } from '../paginacion';
    import { MUNICIPIOS_PUEBLA } from '../municipiosPuebla';

    const TAMANO = 25;
    const GENEROS = ['FEMENINO', 'MASCULINO', 'OTRO', 'NO ESPECIFICADO'];

    type CampoExtra = { nombre: string; valor: string };

    let registros = $state<any[]>([]);
    let total = $state(0);
    let pagina = $state(1);
    let avanzado = $state(false);
    let cargando = $state(false);
    let modal = $state(false);
    let editandoId = $state<string | null>(null);
    let guardando = $state(false);
    let errorFormulario = $state('');
    let filtros = $state({
        buscar: '',
        folio: '',
        curp: '',
        nombre: '',
        genero: '',
        municipio: '',
        fecha_desde: '',
        fecha_hasta: '',
        edad_min: '',
        edad_max: '',
        campo: '',
        valor: ''
    });
    const filtrosActivos = $derived(
        [
            filtros.folio,
            filtros.curp,
            filtros.nombre,
            filtros.genero,
            filtros.municipio,
            filtros.fecha_desde,
            filtros.fecha_hasta,
            filtros.edad_min,
            filtros.edad_max,
            filtros.campo,
            filtros.valor
        ].filter(Boolean).length
    );
    let form = $state({
        fecha: fechaHoy(),
        curp: '',
        nombre: '',
        genero: '',
        municipio: '',
        edad: '',
        observaciones: '',
        campos_extra: [] as CampoExtra[]
    });

    const paginas = $derived(totalPaginas(total, TAMANO));

    onMount(cargar);

    function fechaHoy(): string {
        const hoy = new Date();
        const mes = String(hoy.getMonth() + 1).padStart(2, '0');
        const dia = String(hoy.getDate()).padStart(2, '0');
        return `${hoy.getFullYear()}-${mes}-${dia}`;
    }

    async function cargar() {
        cargando = true;
        const parametros = new URLSearchParams();
        for (const [clave, valor] of Object.entries(filtros)) {
            if (String(valor).trim()) parametros.set(clave, String(valor).trim());
        }
        parametros.set('pagina', String(pagina));
        parametros.set('tamano', String(TAMANO));
        try {
            const respuesta = await api<{ registros: any[]; total: number }>(
                'GET',
                `/api/v1/pa?${parametros.toString()}`
            );
            registros = respuesta.registros;
            total = respuesta.total;
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        } finally {
            cargando = false;
        }
    }

    function buscar() {
        pagina = 1;
        cargar();
    }

    function limpiarFiltros() {
        filtros = {
            buscar: '',
            folio: '',
            curp: '',
            nombre: '',
            genero: '',
            municipio: '',
            fecha_desde: '',
            fecha_hasta: '',
            edad_min: '',
            edad_max: '',
            campo: '',
            valor: ''
        };
        pagina = 1;
        cargar();
    }

    function abrirNuevo() {
        editandoId = null;
        errorFormulario = '';
        form = {
            fecha: fechaHoy(),
            curp: '',
            nombre: '',
            genero: '',
            municipio: '',
            edad: '',
            observaciones: '',
            campos_extra: []
        };
        modal = true;
    }

    function abrirEdicion(registro: any) {
        editandoId = registro.id;
        errorFormulario = '';
        form = {
            fecha: registro.fecha,
            curp: registro.curp,
            nombre: registro.nombre,
            genero: registro.genero,
            municipio: registro.municipio,
            edad: String(registro.edad),
            observaciones: registro.observaciones ?? '',
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

    async function guardar() {
        guardando = true;
        errorFormulario = '';
        try {
            const cuerpo = {
                fecha: form.fecha,
                curp: form.curp,
                nombre: form.nombre,
                genero: form.genero,
                municipio: form.municipio,
                edad: Number(form.edad),
                observaciones: form.observaciones,
                campos_extra: form.campos_extra.filter(
                    (campo) => campo.nombre.trim() !== '' || campo.valor.trim() !== ''
                )
            };
            if (editandoId) {
                await api('PUT', `/api/v1/pa/${editandoId}`, cuerpo);
                app.avisar('Registro P.A. actualizado; constancia regenerada');
            } else {
                await api('POST', '/api/v1/pa', cuerpo);
                app.avisar('Registro P.A. creado; constancia generada');
            }
            modal = false;
            await cargar();
        } catch (fallo) {
            errorFormulario = String(fallo);
        } finally {
            guardando = false;
        }
    }

    async function eliminar(registro: any) {
        if (!confirm(`¿Eliminar el registro ${registro.folio}? Se conservará en auditoría.`)) return;
        try {
            await api('DELETE', `/api/v1/pa/${registro.id}`);
            app.avisar('Registro P.A. eliminado');
            await cargar();
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    async function descargar(registro: any) {
        try {
            const ruta = await invoke<string>('guardar_documento_pa', { id: registro.id });
            app.avisar(`Constancia guardada en ${ruta}`);
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    function fechaVisible(fecha: string): string {
        return new Date(`${fecha}T00:00:00`).toLocaleDateString('es-MX');
    }
</script>

<div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="text-sm text-slate-500">{total} registros P.A.</p>
        <div class="flex items-center gap-1">
            <BotonIcono nombre="actualizar" etiqueta="Actualizar" onclick={cargar} />
            {#if app.permiso('pa:crear')}
                <button class="btn-primario ml-1 px-3 py-1.5 text-xs" type="button" onclick={abrirNuevo}>
                    <Icono nombre="mas" clase="h-3.5 w-3.5" />
                    Nuevo registro
                </button>
            {/if}
        </div>
    </div>

    <section class="tarjeta overflow-hidden">
        <div class="barra-herramientas">
            <div class="relative min-w-0 flex-1 sm:min-w-64">
                <Icono
                    nombre="buscar"
                    clase="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400"
                />
                <input
                    class="campo border-transparent bg-slate-50 pl-9 focus:bg-white"
                    placeholder="Buscar por folio, CURP, nombre, municipio u observaciones"
                    aria-label="Buscar en P.A."
                    bind:value={filtros.buscar}
                    onkeydown={(evento) => evento.key === 'Enter' && buscar()}
                />
            </div>
            <BotonIcono nombre="buscar" etiqueta="Buscar" disabled={cargando} onclick={buscar} />
            <div class="ml-auto flex items-center gap-1">
                <BotonIcono
                    nombre="filtros"
                    etiqueta={avanzado ? 'Ocultar filtros' : 'Filtros avanzados'}
                    activo={avanzado}
                    punto={filtrosActivos > 0 && !avanzado}
                    onclick={() => (avanzado = !avanzado)}
                />
                {#if filtrosActivos > 0}
                    <button
                        class="rounded-md px-2 py-1 text-xs font-medium text-institucional-600 hover:bg-institucional-50"
                        type="button"
                        onclick={limpiarFiltros}
                    >
                        Limpiar
                    </button>
                {/if}
            </div>
        </div>

        {#if avanzado}
            <div class="grid gap-3 border-b border-slate-100 bg-slate-50/60 px-3 py-3 sm:grid-cols-2 md:grid-cols-3">
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Folio</span>
                    <input class="campo py-1.5 text-xs" bind:value={filtros.folio} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">CURP</span>
                    <input class="campo py-1.5 text-xs" bind:value={filtros.curp} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Nombre</span>
                    <input class="campo py-1.5 text-xs" bind:value={filtros.nombre} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Género</span>
                    <select class="campo py-1.5 text-xs" bind:value={filtros.genero}>
                        <option value="">Todos</option>
                        {#each GENEROS as genero}
                            <option value={genero}>{genero}</option>
                        {/each}
                    </select>
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Municipio</span>
                    <input class="campo py-1.5 text-xs" bind:value={filtros.municipio} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Edad mínima</span>
                    <input class="campo py-1.5 text-xs" type="number" min="0" max="150" bind:value={filtros.edad_min} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Edad máxima</span>
                    <input class="campo py-1.5 text-xs" type="number" min="0" max="150" bind:value={filtros.edad_max} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Fecha desde</span>
                    <input class="campo py-1.5 text-xs" type="date" bind:value={filtros.fecha_desde} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Fecha hasta</span>
                    <input class="campo py-1.5 text-xs" type="date" bind:value={filtros.fecha_hasta} />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Campo adicional</span>
                    <input class="campo py-1.5 text-xs" bind:value={filtros.campo} placeholder="Nombre del campo" />
                </label>
                <label class="block">
                    <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Valor del campo</span>
                    <input class="campo py-1.5 text-xs" bind:value={filtros.valor} placeholder="Valor" />
                </label>
                <div class="flex items-end gap-2 sm:col-span-2 md:col-span-3">
                    <button class="btn-secundario px-3 py-1.5 text-xs" type="button" onclick={limpiarFiltros}>
                        Limpiar
                    </button>
                    <button class="btn-primario px-3 py-1.5 text-xs" type="button" onclick={buscar}>
                        Aplicar
                    </button>
                </div>
            </div>
        {/if}

        <div class="max-h-[62vh] overflow-auto">
            <table class="tabla min-w-[900px]">
                <thead>
                    <tr>
                        <th>Folio</th>
                        <th>Fecha</th>
                        <th>Nombre</th>
                        <th>CURP</th>
                        <th>Género</th>
                        <th>Municipio</th>
                        <th>Edad</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    {#each registros as registro (registro.id)}
                        <tr>
                            <td class="font-mono text-xs font-medium text-institucional-600">{registro.folio}</td>
                            <td>{fechaVisible(registro.fecha)}</td>
                            <td>{registro.nombre}</td>
                            <td class="font-mono text-xs">{registro.curp}</td>
                            <td>{registro.genero}</td>
                            <td>{registro.municipio}</td>
                            <td>{registro.edad}</td>
                            <td class="whitespace-nowrap text-right">
                                <div class="flex justify-end gap-1">
                                    <BotonIcono
                                        nombre="pdf"
                                        etiqueta={`Descargar constancia de ${registro.folio}`}
                                        chico
                                        onclick={() => descargar(registro)}
                                    />
                                    {#if app.permiso('pa:editar')}
                                        <BotonIcono
                                            nombre="editar"
                                            etiqueta={`Editar registro ${registro.folio}`}
                                            chico
                                            onclick={() => abrirEdicion(registro)}
                                        />
                                    {/if}
                                    {#if app.permiso('pa:eliminar')}
                                        <BotonIcono
                                            nombre="eliminar"
                                            etiqueta={`Eliminar registro ${registro.folio}`}
                                            chico
                                            peligro
                                            onclick={() => eliminar(registro)}
                                        />
                                    {/if}
                                </div>
                            </td>
                        </tr>
                    {:else}
                        <tr>
                            <td colspan="8" class="py-8 text-center text-sm text-slate-500">
                                No hay registros P.A. con esos filtros.
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
</div>

{#if modal}
    <Modal titulo={editandoId ? 'Editar registro P.A.' : 'Nuevo registro P.A.'} oncerrar={() => (modal = false)}>
        <form
            class="space-y-4"
            onsubmit={(evento) => {
                evento.preventDefault();
                guardar();
            }}
        >
            <div class="grid gap-3 sm:grid-cols-2">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Fecha</span>
                    <input class="campo" type="date" bind:value={form.fecha} required />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">CURP</span>
                    <input
                        class="campo font-mono uppercase"
                        maxlength="18"
                        minlength="18"
                        bind:value={form.curp}
                        required
                    />
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Nombre</span>
                    <input class="campo" maxlength="200" bind:value={form.nombre} required />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Género</span>
                    <select class="campo" bind:value={form.genero} required>
                        <option value="">Seleccione</option>
                        {#each GENEROS as genero}
                            <option value={genero}>{genero}</option>
                        {/each}
                    </select>
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Edad</span>
                    <input class="campo" type="number" min="0" max="150" bind:value={form.edad} required />
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Municipio donde radica</span>
                    <select class="campo" bind:value={form.municipio} required>
                        <option value="">Seleccione</option>
                        {#each MUNICIPIOS_PUEBLA as municipio}
                            <option value={municipio}>{municipio}</option>
                        {/each}
                        {#if form.municipio &&
                        form.municipio !== 'Otros' &&
                        !MUNICIPIOS_PUEBLA.includes(form.municipio)}
                            <option value={form.municipio}>{form.municipio}</option>
                        {/if}
                        <option value="Otros">Otros</option>
                    </select>
                </label>
                <label class="block text-sm sm:col-span-2">
                    <span class="mb-1 block font-medium text-slate-600">Observaciones</span>
                    <textarea class="campo h-24" maxlength="2000" bind:value={form.observaciones}></textarea>
                </label>
            </div>

            <div class="space-y-2">
                <div class="flex items-center justify-between">
                    <span class="text-sm font-medium text-slate-600">Campos adicionales</span>
                    <button class="btn-secundario" type="button" onclick={agregarCampo}>
                        Agregar campo
                    </button>
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
                    {guardando ? 'Guardando…' : 'Guardar y generar PDF'}
                </button>
            </div>
        </form>
    </Modal>
{/if}
