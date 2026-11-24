<script lang="ts">
    import { onMount } from 'svelte';
    import BotonIcono from './BotonIcono.svelte';

    interface Props {
        titulo: string;
        oncerrar: () => void;
        ancho?: number;
        children: any;
    }

    let { titulo, oncerrar, ancho, children }: Props = $props();

    const ENFOCABLES =
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

    let dialogo = $state<HTMLDivElement | undefined>(undefined);

    onMount(() => {
        const anterior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        const primero = dialogo?.querySelector<HTMLElement>(ENFOCABLES);
        (primero ?? dialogo)?.focus();

        return () => anterior?.focus();
    });

    function alPresionarTecla(evento: KeyboardEvent) {
        if (evento.key === 'Escape') {
            evento.preventDefault();
            oncerrar();
            return;
        }
        if (evento.key !== 'Tab' || !dialogo) return;

        const enfocables = Array.from(dialogo.querySelectorAll<HTMLElement>(ENFOCABLES)).filter(
            (elemento) => elemento.offsetParent !== null
        );
        if (enfocables.length === 0) {
            evento.preventDefault();
            dialogo.focus();
            return;
        }

        const primero = enfocables[0];
        const ultimo = enfocables[enfocables.length - 1];
        if (evento.shiftKey && document.activeElement === primero) {
            evento.preventDefault();
            ultimo.focus();
        } else if (!evento.shiftKey && document.activeElement === ultimo) {
            evento.preventDefault();
            primero.focus();
        } else if (!dialogo.contains(document.activeElement)) {
            evento.preventDefault();
            primero.focus();
        }
    }
</script>

<svelte:window onkeydown={alPresionarTecla} />

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
        style:max-width={ancho ? 'min(1400px, 100%)' : undefined}
        class:max-w-3xl={!ancho}
        role="dialog"
        aria-modal="true"
        aria-label={titulo}
        tabindex="-1"
        bind:this={dialogo}
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
