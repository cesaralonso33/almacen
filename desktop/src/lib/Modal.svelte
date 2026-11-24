<script lang="ts">
    import BotonIcono from './BotonIcono.svelte';

    interface Props {
        titulo: string;
        oncerrar: () => void;
        ancho?: number;
        children: any;
    }

    let { titulo, oncerrar, ancho, children }: Props = $props();
</script>

<div
    class="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-slate-900/50 p-2 backdrop-blur-sm sm:p-4"
    role="presentation"
    onclick={(evento) => {
        if (evento.target === evento.currentTarget) oncerrar();
    }}
>
    <div
        class="mt-8 w-full rounded-xl bg-white shadow-2xl ring-1 ring-slate-900/5"
        class:modal-amplio={Boolean(ancho)}
        style:width={ancho ? `${ancho}%` : undefined}
        style:max-width={ancho ? '100%' : undefined}
        class:max-w-3xl={!ancho}
        role="dialog"
        aria-modal="true"
        aria-label={titulo}
    >
        <header class="flex items-center justify-between gap-3 border-b border-slate-200 px-4 py-2.5 sm:px-5">
            <h2 class="truncate text-sm font-semibold text-slate-700">{titulo}</h2>
            <BotonIcono nombre="cerrar" etiqueta="Cerrar" onclick={oncerrar} />
        </header>
        <div class="max-h-[75vh] overflow-y-auto p-4 sm:p-5">
            {@render children?.()}
        </div>
    </div>
</div>
