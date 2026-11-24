<script lang="ts">
    let { nombre, clase = 'h-4 w-4' }: { nombre: string; clase?: string } = $props();
</script>

<svg
    class={clase}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.75"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
>
    {#if nombre === 'buscar'}
        <circle cx="11" cy="11" r="7" />
        <path d="m20 20-3.5-3.5" />
    {:else if nombre === 'filtros'}
        <path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h13M21 18h-1" />
        <circle cx="16" cy="6" r="2" />
        <circle cx="10" cy="12" r="2" />
        <circle cx="19" cy="18" r="2" />
    {:else if nombre === 'columnas'}
        <rect x="3" y="4" width="7" height="16" rx="1" />
        <rect x="14" y="4" width="7" height="16" rx="1" />
    {:else if nombre === 'descargar'}
        <path d="M12 4v10" />
        <path d="m8 10 4 4 4-4" />
        <path d="M5 19h14" />
    {:else if nombre === 'importar'}
        <path d="M12 16V6" />
        <path d="m8 10 4-4 4 4" />
        <path d="M5 19h14" />
    {:else if nombre === 'actualizar'}
        <path d="M20 11a8 8 0 1 0-2.3 6.3" />
        <path d="M20 5v6h-6" />
    {:else if nombre === 'mas'}
        <path d="M12 5v14M5 12h14" />
    {:else if nombre === 'editar'}
        <path d="M12 20h9" />
        <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
    {:else if nombre === 'eliminar'}
        <path d="M4 7h16" />
        <path d="M9 7V4h6v3" />
        <path d="M6 7l1 13h10l1-13" />
        <path d="M10 11v5M14 11v5" />
    {:else if nombre === 'aceptar'}
        <path d="m5 12 4.5 4.5L19 7" />
    {:else if nombre === 'rechazar'}
        <path d="M6 6l12 12M18 6 6 18" />
    {:else if nombre === 'devolver'}
        <path d="M9 14 4 9l5-5" />
        <path d="M4 9h10a6 6 0 0 1 0 12h-3" />
    {:else if nombre === 'transferir'}
        <path d="M4 8h13l-3-3M20 16H7l3 3" />
    {:else if nombre === 'pdf'}
        <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z" />
        <path d="M14 3v5h5" />
        <path d="M9 13h6M9 17h4" />
    {:else if nombre === 'respaldo'}
        <ellipse cx="12" cy="5" rx="8" ry="3" />
        <path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5" />
        <path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" />
    {:else if nombre === 'cerrar'}
        <path d="M6 6l12 12M18 6 6 18" />
    {:else if nombre === 'primera'}
        <path d="M18 6 12 12l6 6M6 6v12" />
    {:else if nombre === 'anterior'}
        <path d="m15 6-6 6 6 6" />
    {:else if nombre === 'siguiente'}
        <path d="m9 6 6 6-6 6" />
    {:else if nombre === 'ultima'}
        <path d="m6 6 6 6-6 6M18 6v12" />
    {:else if nombre === 'menu'}
        <path d="M4 7h16M4 12h16M4 17h16" />
    {:else if nombre === 'sincronizar'}
        <path d="M20 11a8 8 0 0 0-14-5L4 8" />
        <path d="M4 4v4h4" />
        <path d="M4 13a8 8 0 0 0 14 5l2-2" />
        <path d="M20 20v-4h-4" />
    {:else if nombre === 'ojo'}
        <path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6-10-6-10-6Z" />
        <circle cx="12" cy="12" r="2.5" />
    {:else if nombre === 'alerta'}
        <path d="M12 4 2.5 20h19Z" />
        <path d="M12 10v4M12 17h.01" />
    {:else if nombre === 'inicio'}
        <path d="M3 11 12 3l9 8" />
        <path d="M5 10v10h14V10" />
        <path d="M10 20v-5h4v5" />
    {:else if nombre === 'carpeta'}
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
    {:else if nombre === 'persona'}
        <circle cx="12" cy="8" r="3.5" />
        <path d="M5 21a7 7 0 0 1 14 0" />
    {:else if nombre === 'documento'}
        <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z" />
        <path d="M14 3v5h5" />
        <path d="M9 13h6M9 17h4" />
    {:else if nombre === 'lista'}
        <path d="M9 6h11M9 12h11M9 18h11" />
        <path d="M4 6h.01M4 12h.01M4 18h.01" />
    {:else if nombre === 'caja'}
        <path d="M3 8h18v11H3Z" />
        <path d="M3 8l2-4h14l2 4" />
        <path d="M10 12h4" />
    {:else if nombre === 'entrega'}
        <path d="M3 7h10v9H3Z" />
        <path d="M13 10h4l3 3v3h-7Z" />
        <circle cx="7" cy="18" r="1.8" />
        <circle cx="17" cy="18" r="1.8" />
    {:else if nombre === 'candado'}
        <rect x="5" y="11" width="14" height="9" rx="2" />
        <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    {:else if nombre === 'reportes'}
        <path d="M4 20V10M10 20V4M16 20v-7M22 20H2" />
    {:else if nombre === 'usuarios'}
        <circle cx="9" cy="8" r="3" />
        <path d="M3 20a6 6 0 0 1 12 0" />
        <path d="M16 5.5a3 3 0 0 1 0 5.8M17 14a5 5 0 0 1 4 5" />
    {:else if nombre === 'ajustes'}
        <path d="M4 6h16M4 12h16M4 18h16" />
        <circle cx="9" cy="6" r="2" />
        <circle cx="15" cy="12" r="2" />
        <circle cx="7" cy="18" r="2" />
    {/if}
</svg>
