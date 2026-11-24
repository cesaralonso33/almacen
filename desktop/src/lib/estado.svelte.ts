import { invoke } from '@tauri-apps/api/core';
import type { Sincronizacion } from './api';
import { SingleFlight } from './singleFlight.js';

export type SesionInfo = {
    nombre: string;
    usuario_id: string;
    permisos: string[];
    roles: string[];
};

class EstadoApp {
    usuario = $state<SesionInfo | null>(null);
    panel = $state('inicio');
    sync = $state<Sincronizacion | null>(null);
    servidor = $state('');
    mensaje = $state<{ tipo: 'ok' | 'error'; texto: string } | null>(null);
    ocupado = $state(false);
    private sincronizacionEnVuelo = new SingleFlight();

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

    esAdministrador(): boolean {
        return this.usuario?.roles.includes('ADMINISTRADOR') ?? false;
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
        const yaHabiaUna = this.sincronizacionEnVuelo.running;
        try {
            const resultado = await this.sincronizacionEnVuelo.run(async () => {
                let exitosa = false;
                try {
                    await invoke('sincronizar');
                    exitosa = true;
                    if (!silencioso) this.avisar('Sincronización completada');
                } catch (error) {
                    const texto = String(error);
                    if (texto.includes('SESION_EXPIRADA')) {
                        this.cerrarSesionLocal();
                        this.avisar('La sesión expiró; vuelve a iniciar sesión', 'error');
                    } else {
                        this.avisar(`Sin conexión: ${texto}`, 'error');
                    }
                } finally {
                    await this.refrescarSync();
                }
                return { exitosa, notificada: !silencioso };
            });
            if (yaHabiaUna && !silencioso && resultado.exitosa && !resultado.notificada) {
                this.avisar('Sincronización completada');
            }
        } finally {
            this.ocupado = false;
        }
    }
}

export const app = new EstadoApp();
