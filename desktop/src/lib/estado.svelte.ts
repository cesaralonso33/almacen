import { invoke } from '@tauri-apps/api/core';
import type { Sincronizacion } from './api';

export type SesionInfo = {
    nombre: string;
    usuario_id: string;
    permisos: string[];
};

class EstadoApp {
    usuario = $state<SesionInfo | null>(null);
    panel = $state('inicio');
    sync = $state<Sincronizacion | null>(null);
    servidor = $state('');
    mensaje = $state<{ tipo: 'ok' | 'error'; texto: string } | null>(null);
    ocupado = $state(false);

    get inseguro(): boolean {
        return this.servidor.startsWith('http://');
    }

    async cargarServidor() {
        try {
            this.servidor = await invoke<string>('servidor_actual');
        } catch {
            this.servidor = '';
        }
    }

    permiso(clave: string): boolean {
        return this.usuario?.permisos.includes(clave) ?? false;
    }

    avisar(texto: string, tipo: 'ok' | 'error' = 'ok') {
        this.mensaje = { tipo, texto };
        window.setTimeout(() => {
            if (this.mensaje?.texto === texto) this.mensaje = null;
        }, 4500);
    }

    async iniciarSesion(correo: string, password: string) {
        this.ocupado = true;
        try {
            this.usuario = await invoke<SesionInfo>('login', { correo, password });
            await this.cargarServidor();
            await this.refrescarSync();
            await this.sincronizar(true);
        } finally {
            this.ocupado = false;
        }
    }

    async salir() {
        await invoke('logout');
        this.cerrarSesionLocal();
    }

    cerrarSesionLocal() {
        this.usuario = null;
        this.sync = null;
        this.panel = 'inicio';
    }

    async refrescarSync() {
        try {
            this.sync = await invoke<Sincronizacion>('estado_sync');
        } catch {
            this.sync = null;
        }
    }

    async sincronizar(silencioso = false) {
        this.ocupado = true;
        try {
            await invoke('sincronizar');
            if (!silencioso) this.avisar('Sincronización completada');
        } catch (error) {
            this.avisar(`Sin conexión: ${String(error)}`, 'error');
        } finally {
            this.ocupado = false;
            await this.refrescarSync();
        }
    }
}

export const app = new EstadoApp();
