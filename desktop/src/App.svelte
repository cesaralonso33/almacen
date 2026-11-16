<script lang="ts">
 import { invoke } from '@tauri-apps/api/core';
 let numero = $state('');
 let descripcion = $state('');
 let expedientes = $state<{id:string;numero:string;descripcion:string;estado:string}[]>([]);
 let mensaje = $state('');
 async function cargar(){ try {expedientes=await invoke('listar_expedientes',{filtro:numero});}catch(e){mensaje=String(e);} }
 async function agregar(){try{await invoke('crear_expediente_local',{numero,descripcion});numero='';descripcion='';await cargar();mensaje='Expediente local creado; pendiente de sincronización';}catch(e){mensaje=String(e);}}
 cargar();
</script>
<svelte:head><title>Almacén jurídico</title></svelte:head>
<main>
 <h1>Almacén y custodia</h1><p>Inventario local · SQLite</p>
 <section><input placeholder="Número de expediente" bind:value={numero} oninput={cargar}/><input placeholder="Descripción" bind:value={descripcion}/><button onclick={agregar}>Agregar local</button></section>
 <p role="status">{mensaje}</p>
 <table><thead><tr><th>Número</th><th>Descripción</th><th>Estado</th></tr></thead><tbody>{#each expedientes as e}<tr><td>{e.numero}</td><td>{e.descripcion}</td><td>{e.estado}</td></tr>{/each}</tbody></table>
</main>
<style>
 :global(body){font-family:system-ui;background:#f5f7fa;color:#17212e;margin:0}main{max-width:1100px;margin:3rem auto;padding:2rem;background:white;border-radius:14px;box-shadow:0 3px 20px #0001}h1{margin:0}section{display:flex;gap:10px;margin:24px 0}input,button{padding:12px;border:1px solid #d2d8e0;border-radius:7px}button{background:#1b4777;color:white;cursor:pointer}table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:13px;border-bottom:1px solid #e6e9ef}
</style>
