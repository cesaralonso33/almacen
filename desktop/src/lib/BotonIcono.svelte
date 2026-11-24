<script lang="ts">
    import Icono from './Icono.svelte';

    interface Props {
        nombre: string;
        etiqueta: string;
        onclick?: (evento: MouseEvent) => void;
        activo?: boolean;
        peligro?: boolean;
        chico?: boolean;
        disabled?: boolean;
        tipo?: 'button' | 'submit';
        extra?: string;
        punto?: boolean;
    }

    let {
        nombre,
        etiqueta,
        onclick,
        activo = false,
        peligro = false,
        chico = false,
        disabled = false,
        tipo = 'button',
        extra = '',
        punto = false
    }: Props = $props();
</script>

<button
    class="boton-icono relative {chico ? 'h-7 w-7' : ''} {activo ? 'boton-icono-activo' : ''} {peligro
        ? 'hover:bg-rose-50 hover:text-rose-600'
        : ''} {extra}"
    type={tipo}
    {disabled}
    onclick={onclick}
    aria-label={etiqueta}
    title={etiqueta}
    aria-pressed={activo || undefined}
>
    <Icono {nombre} clase={chico ? 'h-3.5 w-3.5' : 'h-4 w-4'} />
    {#if punto}
        <span
            class="absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full bg-institucional-500"
            aria-hidden="true"
        ></span>
    {/if}
</button>
