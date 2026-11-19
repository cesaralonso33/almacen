<script lang="ts">
    interface Props {
        titulo: string;
        oncerrar: () => void;
        ancho?: number;
        children: any;
    }

    let { titulo, oncerrar, ancho, children }: Props = $props();
</script>

<div
    class="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-slate-900/50 p-4"
    role="presentation"
    onclick={(evento) => {
        if (evento.target === evento.currentTarget) oncerrar();
    }}
>
    <div
        class="mt-8 w-full rounded-xl bg-white shadow-2xl"
        style:width={ancho ? `${ancho}%` : undefined}
        style:max-width={ancho ? 'none' : undefined}
        class:max-w-3xl={!ancho}
        role="dialog"
        aria-modal="true"
        aria-label={titulo}
    >
        <header class="flex items-center justify-between border-b border-slate-200 px-5 py-3">
            <h2 class="text-sm font-semibold uppercase tracking-wide text-slate-600">{titulo}</h2>
            <button class="btn-secundario" type="button" onclick={oncerrar}>Cerrar</button>
        </header>
        <div class="max-h-[75vh] overflow-y-auto p-5">
            {@render children?.()}
        </div>
    </div>
</div>
