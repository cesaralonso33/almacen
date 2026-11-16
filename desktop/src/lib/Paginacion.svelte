<script lang="ts">
    interface Props {
        pagina: number;
        paginas: number;
        total: number;
        onCambiar: (pagina: number) => void;
    }

    let { pagina, paginas, total, onCambiar }: Props = $props();
</script>

{#if total > 0}
    <div
        class="flex flex-wrap items-center justify-between gap-2 border-t border-slate-100 bg-slate-50/60 px-3 py-2 text-xs text-slate-500"
    >
        <span>{total} registros · página {pagina} de {paginas}</span>
        <div class="flex items-center gap-1">
            <button
                class="btn-secundario px-2 py-1"
                disabled={pagina <= 1}
                onclick={() => onCambiar(1)}
                aria-label="Primera página"
            >
                «
            </button>
            <button
                class="btn-secundario px-2 py-1"
                disabled={pagina <= 1}
                onclick={() => onCambiar(pagina - 1)}
                aria-label="Página anterior"
            >
                ‹
            </button>
            <button
                class="btn-secundario px-2 py-1"
                disabled={pagina >= paginas}
                onclick={() => onCambiar(pagina + 1)}
                aria-label="Página siguiente"
            >
                ›
            </button>
            <button
                class="btn-secundario px-2 py-1"
                disabled={pagina >= paginas}
                onclick={() => onCambiar(paginas)}
                aria-label="Última página"
            >
                »
            </button>
        </div>
    </div>
{/if}
