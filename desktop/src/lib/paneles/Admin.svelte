<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '../api';
    import { app } from '../estado.svelte';

    let usuarios = $state<any[]>([]);
    let roles = $state<any[]>([]);
    let creando = $state(false);
    let nombre = $state('');
    let correo = $state('');
    let password = $state('');
    let seleccionRoles = $state<string[]>([]);

    onMount(cargar);

    async function cargar() {
        try {
            const lista = await api<{ usuarios: any[] }>('GET', '/api/v1/admin/usuarios');
            usuarios = lista.usuarios;
            const catalogo = await api<{ roles: any[] }>('GET', '/api/v1/admin/roles');
            roles = catalogo.roles;
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    function alternarRol(clave: string) {
        seleccionRoles = seleccionRoles.includes(clave)
            ? seleccionRoles.filter((valor) => valor !== clave)
            : [...seleccionRoles, clave];
    }

    async function crear() {
        try {
            await api('POST', '/api/v1/admin/usuarios', {
                nombre,
                correo,
                password,
                roles: seleccionRoles
            });
            app.avisar('Usuario creado; deberá cambiar la contraseña al entrar');
            creando = false;
            nombre = correo = password = '';
            seleccionRoles = [];
            await cargar();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }

    async function cambiarEstado(usuario: any) {
        const accion = usuario.activo ? 'desactivar' : 'activar';
        try {
            await api('POST', `/api/v1/admin/usuarios/${usuario.id}/${accion}`, {});
            app.avisar(`Usuario ${accion === 'activar' ? 'activado' : 'desactivado'}`);
            await cargar();
        } catch (error) {
            app.avisar(String(error), 'error');
        }
    }
</script>

<div class="space-y-6">
    <div class="flex items-center justify-between">
        <p class="text-sm text-slate-500">{usuarios.length} usuarios</p>
        <button class="btn-primario" onclick={() => (creando = !creando)}>Nuevo usuario</button>
    </div>

    {#if creando}
        <section class="tarjeta space-y-3 p-4">
            <div class="grid gap-3 sm:grid-cols-3">
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Nombre</span>
                    <input class="campo" bind:value={nombre} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Correo</span>
                    <input class="campo" type="email" bind:value={correo} />
                </label>
                <label class="block text-sm">
                    <span class="mb-1 block font-medium text-slate-600">Contraseña inicial</span>
                    <input class="campo" type="password" bind:value={password} />
                </label>
            </div>
            <div class="flex flex-wrap gap-3">
                {#each roles as rol}
                    <label class="flex items-center gap-2 rounded-lg border border-slate-200 px-3 py-2 text-sm">
                        <input
                            type="checkbox"
                            checked={seleccionRoles.includes(rol.clave)}
                            onchange={() => alternarRol(rol.clave)}
                        />
                        {rol.clave}
                    </label>
                {/each}
            </div>
            <button class="btn-primario" onclick={crear}>Crear usuario</button>
        </section>
    {/if}

    <section class="tarjeta overflow-hidden">
        <table class="tabla">
            <thead>
                <tr>
                    <th>Nombre</th>
                    <th>Correo</th>
                    <th>Roles</th>
                    <th>Estado</th>
                    <th></th>
                </tr>
            </thead>
            <tbody>
                {#each usuarios as usuario}
                    <tr>
                        <td class="font-medium">{usuario.nombre}</td>
                        <td class="text-slate-600">{usuario.correo}</td>
                        <td class="text-slate-500">{usuario.roles.join(', ')}</td>
                        <td>
                            <span class="insignia {usuario.activo ? 'bg-emerald-100 text-emerald-700' : 'bg-rose-100 text-rose-700'}">
                                {usuario.activo ? 'Activo' : 'Inactivo'}
                            </span>
                        </td>
                        <td class="text-right">
                            <button class="btn-secundario" onclick={() => cambiarEstado(usuario)}>
                                {usuario.activo ? 'Desactivar' : 'Activar'}
                            </button>
                        </td>
                    </tr>
                {/each}
            </tbody>
        </table>
    </section>

    <section class="tarjeta p-4">
        <h2 class="mb-3 text-sm font-semibold text-slate-700">Roles y permisos</h2>
        <div class="grid gap-3 md:grid-cols-2">
            {#each roles as rol}
                <div class="rounded-lg border border-slate-200 p-3">
                    <p class="text-sm font-semibold text-slate-700">{rol.clave}</p>
                    <p class="text-xs text-slate-500">{rol.nombre}</p>
                    <p class="mt-2 text-xs text-slate-500">{rol.permisos.length} permisos</p>
                </div>
            {/each}
        </div>
    </section>
</div>
