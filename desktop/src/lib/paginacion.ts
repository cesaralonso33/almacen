/// Utilidades de paginación en memoria para las tablas de la interfaz.
export const POR_PAGINA = 15;

export function totalPaginas(total: number, tamano = POR_PAGINA): number {
    return Math.max(1, Math.ceil(total / tamano));
}

export function recortar<T>(filas: T[], pagina: number, tamano = POR_PAGINA): T[] {
    const inicio = (pagina - 1) * tamano;
    return filas.slice(inicio, inicio + tamano);
}
