<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api } from '../api';
    import { app } from '../estado.svelte';
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
    <section class="tarjeta space-y-3 p-4">
        <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
                <h2 class="text-base font-semibold text-slate-800">P.A.</h2>
                <p class="text-sm text-slate-500">
                    Registro de personas con folio anual y constancia PDF.
                </p>
            </div>
            <div class="flex flex-wrap gap-2">
                <button class="btn-secundario" type="button" onclick={() => (avanzado = !avanzado)}>
                    {avanzado ? 'Ocultar filtros' : 'Filtros avanzados'}
                </button>
                {#if app.permiso('pa:crear')}
                    <button class="btn-primario" type="button" onclick={abrirNuevo}>Nuevo registro</button>
                {/if}
            </div>
        </div>

        <div class="flex flex-wrap gap-2">
            <input
                class="campo min-w-0 flex-1 sm:min-w-64"
                placeholder="Buscar por folio, CURP, nombre, municipio u observaciones"
                bind:value={filtros.buscar}
                onkeydown={(evento) => evento.key === 'Enter' && buscar()}
            />
            <button class="btn-secundario" type="button" onclick={buscar} disabled={cargando}>
                {cargando ? 'Buscando…' : 'Buscar'}
            </button>
        </div>

        {#if avanzado}
            <div class="grid gap-3 md:grid-cols-3">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Folio</span>
                    <input class="campo" bind:value={filtros.folio} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">CURP</span>
                    <input class="campo" bind:value={filtros.curp} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Nombre</span>
                    <input class="campo" bind:value={filtros.nombre} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Género</span>
                    <select class="campo" bind:value={filtros.genero}>
                        <option value="">Todos</option>
                        {#each GENEROS as genero}
                            <option value={genero}>{genero}</option>
                        {/each}
                    </select>
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Municipio</span>
                    <input class="campo" bind:value={filtros.municipio} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Edad mínima</span>
                    <input class="campo" type="number" min="0" max="150" bind:value={filtros.edad_min} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Edad máxima</span>
                    <input class="campo" type="number" min="0" max="150" bind:value={filtros.edad_max} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Fecha desde</span>
                    <input class="campo" type="date" bind:value={filtros.fecha_desde} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Fecha hasta</span>
                    <input class="campo" type="date" bind:value={filtros.fecha_hasta} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Campo adicional</span>
                    <input class="campo" bind:value={filtros.campo} placeholder="Nombre del campo" />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Valor del campo</span>
                    <input class="campo" bind:value={filtros.valor} placeholder="Valor" />
                </label>
                <div class="flex items-end gap-2">
                    <button class="btn-secundario" type="button" onclick={limpiarFiltros}>Limpiar</button>
                    <button class="btn-primario" type="button" onclick={buscar}>Aplicar</button>
                </div>
            </div>
        {/if}
    </section>

    <section class="tarjeta overflow-hidden">
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
                            <td class="space-x-2 whitespace-nowrap text-right">
                                <button class="btn-secundario" type="button" onclick={() => descargar(registro)}>
                                    PDF
                                </button>
                                {#if app.permiso('pa:editar')}
                                    <button class="btn-secundario" type="button" onclick={() => abrirEdicion(registro)}>
                                        Editar
                                    </button>
                                {/if}
                                {#if app.permiso('pa:eliminar')}
                                    <button class="btn-secundario" type="button" onclick={() => eliminar(registro)}>
                                        Eliminar
                                    </button>
                                {/if}
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
