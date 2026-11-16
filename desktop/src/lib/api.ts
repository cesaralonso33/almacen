import { invoke } from '@tauri-apps/api/core';

/// Llama a la API a través del proceso principal de Tauri, que conserva el token en memoria.
export async function api<T = any>(
    metodo: 'GET' | 'POST',
    ruta: string,
    cuerpo?: unknown
): Promise<T> {
    return invoke<T>('peticion', { metodo, ruta, cuerpo: cuerpo ?? null });
}

export type Sincronizacion = {
    estado: string;
    pendientes: number;
    conflictos: number;
    cursor: number;
    ultima_sync: string | null;
    ultimo_error: string | null;
    usuario: string | null;
};

export type Expediente = {
    id: string;
    numero: string;
    descripcion: string;
    estado: string;
    custodio: string | null;
    version: number;
};

export type PedidoResumen = {
    id: string;
    solicitante_id: string;
    solicitante_nombre: string;
    creado_en: string;
    estado: string;
    total: number;
    entregados: number;
    descartados: number;
    cancelados: number;
};

export type ItemPedido = {
    id: string;
    estado: string;
    expediente_id: string;
};
