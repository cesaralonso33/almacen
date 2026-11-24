<script lang="ts">
    import BotonIcono from './BotonIcono.svelte';

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
        class="flex flex-wrap items-center justify-between gap-2 border-t border-slate-100 bg-slate-50/60 px-3 py-1.5 text-xs text-slate-500"
    >
        <span>{total} registros · página {pagina} de {paginas}</span>
        <div class="flex items-center gap-0.5">
            <BotonIcono
                nombre="primera"
                etiqueta="Primera página"
                chico
                disabled={pagina <= 1}
                onclick={() => onCambiar(1)}
            />
            <BotonIcono
                nombre="anterior"
                etiqueta="Página anterior"
                chico
                disabled={pagina <= 1}
                onclick={() => onCambiar(pagina - 1)}
            />
            <BotonIcono
                nombre="siguiente"
                etiqueta="Página siguiente"
                chico
                disabled={pagina >= paginas}
                onclick={() => onCambiar(pagina + 1)}
            />
            <BotonIcono
                nombre="ultima"
                etiqueta="Última página"
                chico
                disabled={pagina >= paginas}
                onclick={() => onCambiar(paginas)}
            />
        </div>
    </div>
{/if}
