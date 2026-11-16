<script lang="ts">
    let { estado }: { estado: string } = $props();

    const clases: Record<string, string> = {
        PENDIENTE: 'bg-amber-100 text-amber-700',
        EN_PROCESO: 'bg-sky-100 text-sky-700',
        PARCIAL: 'bg-indigo-100 text-indigo-700',
        COMPLETADO: 'bg-emerald-100 text-emerald-700',
        CANCELADO: 'bg-slate-200 text-slate-600',
        PREPARADO: 'bg-sky-100 text-sky-700',
        RESERVADO: 'bg-sky-100 text-sky-700',
        EN_ENTREGA: 'bg-purple-100 text-purple-700',
        ENTREGADO: 'bg-emerald-100 text-emerald-700',
        DESCARTADO: 'bg-rose-100 text-rose-700',
        DESPACHADA: 'bg-purple-100 text-purple-700',
        RECIBIDA: 'bg-emerald-100 text-emerald-700',
        ALMACEN: 'bg-slate-100 text-slate-600',
        EN_CUSTODIA: 'bg-indigo-100 text-indigo-700',
        NO_LOCALIZADO: 'bg-rose-100 text-rose-700',
        CONFIRMADA: 'bg-emerald-100 text-emerald-700',
        SOLICITADA: 'bg-amber-100 text-amber-700'
    };
</script>

<span class="insignia {clases[estado] ?? 'bg-slate-100 text-slate-600'}">
    {estado.replaceAll('_', ' ')}
</span>
