<script lang="ts">
    import { onMount } from 'svelte';
    import { api, type Expediente } from '../api';
    import { app } from '../estado.svelte';
    import Insignia from '../Insignia.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { totalPaginas } from '../paginacion';

    const TAMANO = 25;

    let usuarios = $state<any[]>([]);
    let destino = $state('');
    let busqueda = $state('');
    let estadoFiltro = $state('ALMACEN');
    let resultados = $state<Expediente[]>([]);
    let total = $state(0);
    let pagina = $state(1);
    let seleccion = $state<string[]>([]);
    let buscando = $state(false);
    let enviando = $state(false);
    let resultado = $state<any>(null);

    const paginas = $derived(totalPaginas(total, TAMANO));

    onMount(async () => {
        await cargarUsuarios();
        await buscar();
    });

    async function cargarUsuarios() {
        try {
            const respuesta = await api<{ usuarios: any[] }>('GET', '/api/v1/admin/usuarios');
            usuarios = respuesta.usuarios
                .filter((usuario) => usuario.activo)
                .sort((a, b) => a.nombre.localeCompare(b.nombre));
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function buscar() {
        buscando = true;
        try {
            const parametros = new URLSearchParams();
            if (busqueda.trim()) parametros.set('buscar', busqueda.trim());
            if (estadoFiltro) parametros.set('estado', estadoFiltro);
            parametros.set('pagina', String(pagina));
            parametros.set('tamano', String(TAMANO));
            const respuesta = await api<any>(
                'GET',
                `/api/v1/expedientes?${parametros.toString()}`
            );
            resultados = respuesta.expedientes;
            total = respuesta.total;
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            buscando = false;
        }
    }

    function nuevaBusqueda() {
        pagina = 1;
        buscar();
    }

    function alternar(id: string) {
        seleccion = seleccion.includes(id)
            ? seleccion.filter((valor) => valor !== id)
            : [...seleccion, id];
    }

    function limpiarSeleccion() {
        seleccion = [];
    }

    async function asignar() {
        if (!destino || seleccion.length === 0) return;
        enviando = true;
        resultado = null;
        try {
            resultado = await api('POST', '/api/v1/pedidos/asignar', {
                destino_id: destino,
                expedientes: seleccion
            });
            app.avisar(`Pedido creado con ${seleccion.length} expediente(s)`);
            seleccion = [];
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            enviando = false;
        }
    }

    function nombreDestino(): string {
        return usuarios.find((usuario) => usuario.id === destino)?.nombre ?? '';
    }
</script>

<div class="space-y-4">
    <section class="tarjeta p-4">
        <div class="flex flex-wrap items-start justify-between gap-3">
            <div>
                <h2 class="text-base font-semibold text-slate-800">Asignación masiva de pedidos</h2>
                <p class="mt-1 max-w-2xl text-sm text-slate-500">
                    Crea un pedido a nombre del usuario seleccionado. La custodia no cambia aquí: almacén
                    prepara y el destinatario confirma la recepción.
                </p>
            </div>
            {#if seleccion.length > 0}
                <span class="insignia bg-indigo-100 text-indigo-700">
                    {seleccion.length} seleccionado(s)
                </span>
            {/if}
        </div>

        <div class="mt-4 grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
            <label class="block text-sm">
                <span class="mb-1 block font-medium text-slate-600">Usuario destinatario</span>
                <select class="campo" bind:value={destino}>
                    <option value="">Seleccione un usuario</option>
                    {#each usuarios as usuario (usuario.id)}
                        <option value={usuario.id}>
                            {usuario.nombre}{usuario.correo ? ` · ${usuario.correo}` : ''}
                        </option>
                    {/each}
                </select>
            </label>

            <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem_auto]">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Buscar expediente</span>
                    <input
                        class="campo"
                        bind:value={busqueda}
                        placeholder="Número o descripción"
                        onkeydown={(evento) => evento.key === 'Enter' && nuevaBusqueda()}
                    />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Disponibilidad</span>
                    <select class="campo" bind:value={estadoFiltro} onchange={nuevaBusqueda}>
                        <option value="ALMACEN">En almacén</option>
                        <option value="">Todos los estados</option>
                    </select>
                </label>
                <div class="flex items-end">
                    <button class="btn-secundario" type="button" onclick={nuevaBusqueda} disabled={buscando}>
                        {buscando ? 'Buscando…' : 'Buscar'}
                    </button>
                </div>
            </div>
        </div>
    </section>

    <section class="tarjeta overflow-hidden">
        <div class="overflow-x-auto">
            <table class="tabla min-w-[800px]">
                <thead>
                    <tr>
                        <th class="w-10"></th>
                        <th>Número</th>
                        <th>Descripción</th>
                        <th>Estado</th>
                        <th>Custodio</th>
                    </tr>
                </thead>
                <tbody>
                    {#each resultados as expediente (expediente.id)}
                        <tr>
                            <td>
                                <input
                                    type="checkbox"
                                    aria-label={`Seleccionar ${expediente.numero}`}
                                    checked={seleccion.includes(expediente.id)}
                                    onchange={() => alternar(expediente.id)}
                                />
                            </td>
                            <td class="font-mono text-xs">{expediente.numero}</td>
                            <td>{expediente.descripcion || 'Sin descripción'}</td>
                            <td><Insignia estado={expediente.estado} /></td>
                            <td>{expediente.custodio || 'Almacén'}</td>
                        </tr>
                    {:else}
                        <tr>
                            <td colspan="5" class="py-8 text-center text-sm text-slate-500">
                                No hay expedientes para los filtros seleccionados.
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
                buscar();
            }}
        />
    </section>

    <section class="tarjeta flex flex-wrap items-center justify-between gap-3 p-4">
        <div class="text-sm text-slate-600">
            {#if destino}
                Se creará un pedido para <strong>{nombreDestino()}</strong> con
                <strong>{seleccion.length}</strong> expediente(s).
            {:else}
                Seleccione el usuario destinatario y los expedientes.
            {/if}
        </div>
        <div class="flex items-center gap-2">
            {#if seleccion.length > 0}
                <button class="btn-secundario" type="button" onclick={limpiarSeleccion}>
                    Limpiar selección
                </button>
            {/if}
            <button
                class="btn-primario"
                type="button"
                onclick={asignar}
                disabled={!destino || seleccion.length === 0 || enviando}
            >
                {enviando ? 'Creando pedido…' : 'Crear pedido'}
            </button>
        </div>
    </section>

    {#if resultado}
        <section class="tarjeta border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-800">
            Pedido <span class="font-mono">{resultado.pedido_id}</span> creado con
            {resultado.total} expediente(s). Almacén lo verá en su bandeja y el destinatario
            deberá confirmar la recepción.
        </section>
    {/if}
</div>
