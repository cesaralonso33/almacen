<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { api } from '../api';
    import { app } from '../estado.svelte';
    import Modal from '../Modal.svelte';
    import Paginacion from '../Paginacion.svelte';
    import { totalPaginas } from '../paginacion';

    const TAMANO = 25;
    const ESTATUS = ['PENDIENTE', 'INCOMPLETO', 'COMPLETO'];
    const TIPOS = [
        'CANASTA',
        'REEMBOLSO',
        'APOYO PARA VIÁTICOS',
        'MEDICAMENTO',
        'OTRAS AYUDAS',
        'VIATICOS',
        'AYUDAS EN MATERIA DE SALUD',
        'GASTOS DE TRASLADO, HOSPEDAJE (VIATICOS)'
    ];
    const CAMPOS_DOCUMENTO = [
        { clave: 'CURP', etiqueta: 'CURP' },
        { clave: 'RFC', etiqueta: 'RFC' },
        { clave: 'REGISTRO', etiqueta: 'Registro' },
        { clave: 'IDENTIFICACION', etiqueta: 'Identificación' },
        { clave: 'ESTADO_CUENTA', etiqueta: 'Estado de cuenta' },
        { clave: 'DEPOSITO', etiqueta: 'Depósito' },
        { clave: 'ESCRITO_LIBRE', etiqueta: 'Escrito libre' }
    ];

    const COLUMNAS = [
        { clave: 'numero_darfe', etiqueta: 'No. DARFE' },
        { clave: 'fecha', etiqueta: 'Fecha' },
        { clave: 'nombre', etiqueta: 'Nombre' },
        { clave: 'expediente', etiqueta: 'Número de expediente' },
        { clave: 'descripcion', etiqueta: 'Descripción' },
        { clave: 'monto', etiqueta: 'Monto' },
        { clave: 'numero_cuenta', etiqueta: 'Número de cuenta' },
        { clave: 'tipo', etiqueta: 'Tipo' },
        { clave: 'banco', etiqueta: 'Banco' },
        { clave: 'estatus', etiqueta: 'Estatus' },
        { clave: 'documentos', etiqueta: 'Documentos' }
    ] as const;

    type ClaveColumna = (typeof COLUMNAS)[number]['clave'];

    const PESTANAS_FORM = [
        { clave: 'generales', etiqueta: 'Datos generales' },
        { clave: 'pago', etiqueta: 'Pago' },
        { clave: 'contacto', etiqueta: 'Contacto' },
        { clave: 'documentos', etiqueta: 'Documentos' },
        { clave: 'campos', etiqueta: 'Campos adicionales' }
    ] as const;

    type PestanaForm = (typeof PESTANAS_FORM)[number]['clave'];

    const COLUMNAS_PREDETERMINADAS: Record<ClaveColumna, boolean> = {
        numero_darfe: true,
        fecha: true,
        nombre: true,
        expediente: true,
        descripcion: true,
        monto: true,
        numero_cuenta: true,
        tipo: true,
        banco: true,
        estatus: true,
        documentos: true
    };

    type CampoExtra = { nombre: string; valor: string };
    type Archivos = Record<string, File | null>;

    let registros = $state<any[]>([]);
    let total = $state(0);
    let pagina = $state(1);
    let pestana = $state<'registros' | 'reportes'>('registros');
    let avanzado = $state(false);
    let cargando = $state(false);
    let filtros = $state({
        buscar: '',
        fecha_desde: '',
        fecha_hasta: '',
        estatus: '',
        banco: '',
        tipo: ''
    });
    let reportes = $state<any>(null);
    let modal = $state(false);
    let editandoId = $state<string | null>(null);
    let guardando = $state(false);
    let errorFormulario = $state('');
    let documentosExistentes = $state<string[]>([]);
    let archivos = $state<Archivos>(vacioArchivos());
    let vistaDocumento = $state<{
        id: string;
        campo: string;
        nombre: string;
        mime: string;
        url: string;
    } | null>(null);
    let folioFormulario = $state('');
    let guardandoFolio = $state(false);
    let pestanaForm = $state<PestanaForm>('generales');
    let consultandoExpediente = $state(false);
    let avisoConsulta = $state('');
    let paperlessId = $state('');
    let paperlessCampo = $state('CURP');
    let importandoPaperless = $state(false);
    let auditoriaAbierta = $state(false);
    let auditoriaCargando = $state(false);
    let auditoriaError = $state('');
    let auditoriaDarfe = $state<{ id: string; numero_darfe: string } | null>(null);
    let auditoriaEventos = $state<any[]>([]);
    let columnas = $state<Record<ClaveColumna, boolean>>({ ...COLUMNAS_PREDETERMINADAS });
    let menuColumnas = $state(false);
    let menuColumnasEl = $state<HTMLDivElement | undefined>(undefined);
    let form = $state({
        numero_darfe: '',
        fecha: fechaHoy(),
        nombre: '',
        expediente: '',
        registro: '',
        banco: '',
        curp: '',
        rfc: '',
        telefono: '',
        correo: '',
        descripcion: '',
        monto: '',
        numero_cuenta: '',
        tipo: 'REEMBOLSO',
        estatus: 'PENDIENTE',
        campos_extra: [] as CampoExtra[]
    });

    const paginas = $derived(totalPaginas(total, TAMANO));
    const columnasVisibles = $derived(
        COLUMNAS.filter((columna) => columnas[columna.clave]).length
    );
    const filtrosAvanzados = $derived(
        [filtros.fecha_desde, filtros.fecha_hasta, filtros.estatus, filtros.banco, filtros.tipo].filter(
            (valor) => Boolean(valor)
        ).length
    );

    onMount(() => {
        cargarColumnas();
        cargar();
    });

    function fechaHoy(): string {
        const hoy = new Date();
        return `${hoy.getFullYear()}-${String(hoy.getMonth() + 1).padStart(2, '0')}-${String(
            hoy.getDate()
        ).padStart(2, '0')}`;
    }

    function vacioArchivos(): Archivos {
        return Object.fromEntries(CAMPOS_DOCUMENTO.map((campo) => [campo.clave, null]));
    }

    function parametros(): string {
        const params = new URLSearchParams();
        for (const [clave, valor] of Object.entries(filtros)) {
            if (String(valor).trim()) params.set(clave, String(valor).trim());
        }
        params.set('pagina', String(pagina));
        params.set('tamano', String(TAMANO));
        return params.toString();
    }

    async function cargar() {
        if (pestana === 'reportes') {
            await cargarReportes();
            return;
        }
        cargando = true;
        try {
            const respuesta = await api<{ darfes: any[]; total: number }>(
                'GET',
                `/api/v1/darfes?${parametros()}`
            );
            registros = respuesta.darfes;
            total = respuesta.total;
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        } finally {
            cargando = false;
        }
    }

    async function cargarReportes() {
        try {
            reportes = await api('GET', '/api/v1/darfes/reportes');
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    function buscar() {
        pagina = 1;
        cargar();
    }

    function limpiarFiltros() {
        filtros = { buscar: '', fecha_desde: '', fecha_hasta: '', estatus: '', banco: '', tipo: '' };
        pagina = 1;
        cargar();
    }

    function abrirNuevo() {
        editandoId = null;
        folioFormulario = '';
        pestanaForm = 'generales';
        avisoConsulta = '';
        paperlessId = '';
        documentosExistentes = [];
        archivos = vacioArchivos();
        errorFormulario = '';
        form = {
            numero_darfe: '',
            fecha: fechaHoy(),
            nombre: '',
            expediente: '',
            registro: '',
            banco: '',
            curp: '',
            rfc: '',
            telefono: '',
            correo: '',
            descripcion: '',
            monto: '',
            numero_cuenta: '',
            tipo: 'REEMBOLSO',
            estatus: 'PENDIENTE',
            campos_extra: []
        };
        modal = true;
    }

    function duplicar() {
        editandoId = null;
        pestanaForm = 'generales';
        avisoConsulta = '';
        paperlessId = '';
        documentosExistentes = [];
        archivos = vacioArchivos();
        errorFormulario = '';
        form = { ...form, numero_darfe: '' };
    }

    function abrirEdicion(registro: any) {
        editandoId = registro.id;
        pestanaForm = 'generales';
        avisoConsulta = '';
        paperlessId = '';
        folioFormulario = registro.folio_formato ? String(registro.folio_formato) : '';
        documentosExistentes = Array.isArray(registro.documentos) ? registro.documentos : [];
        archivos = vacioArchivos();
        errorFormulario = '';
        form = {
            numero_darfe: registro.numero_darfe,
            fecha: registro.fecha,
            nombre: registro.nombre,
            expediente: registro.expediente ?? '',
            registro: registro.registro ?? '',
            banco: registro.banco ?? '',
            curp: registro.curp ?? '',
            rfc: registro.rfc ?? '',
            telefono: registro.telefono ?? '',
            correo: registro.correo ?? '',
            descripcion: registro.descripcion ?? '',
            monto: String(registro.monto ?? ''),
            numero_cuenta: registro.numero_cuenta ?? '',
            tipo: registro.tipo ?? 'REEMBOLSO',
            estatus: registro.estatus,
            campos_extra: Array.isArray(registro.campos_extra)
                ? registro.campos_extra.map((campo: any) => ({ nombre: campo.nombre, valor: campo.valor }))
                : []
        };
        modal = true;
    }

    async function abrirAuditoria(registro: any) {
        if (!app.esAdministrador()) return;
        auditoriaAbierta = true;
        auditoriaCargando = true;
        auditoriaError = '';
        auditoriaEventos = [];
        auditoriaDarfe = { id: registro.id, numero_darfe: registro.numero_darfe };
        try {
            const respuesta = await api<{
                darfe: { id: string; numero_darfe: string };
                eventos: any[];
            }>('GET', `/api/v1/darfes/${registro.id}/auditoria`);
            auditoriaDarfe = respuesta.darfe;
            auditoriaEventos = respuesta.eventos;
        } catch (fallo) {
            auditoriaError = String(fallo);
        } finally {
            auditoriaCargando = false;
        }
    }

    function cerrarAuditoria() {
        auditoriaAbierta = false;
        auditoriaEventos = [];
        auditoriaError = '';
        auditoriaDarfe = null;
    }

    const ETIQUETAS_CAMPO: Record<string, string> = {
        numero_darfe: 'No. DARFE',
        fecha: 'Fecha',
        nombre: 'Nombre',
        expediente: 'Número de expediente',
        registro: 'Registro',
        banco: 'Banco',
        curp: 'CURP',
        rfc: 'RFC',
        telefono: 'Teléfono',
        correo: 'Correo',
        descripcion: 'Descripción',
        monto: 'Monto',
        numero_cuenta: 'Número de cuenta',
        estatus: 'Estatus',
        campos_extra: 'Campos adicionales'
    };

    function etiquetaCampo(campo: string): string {
        return ETIQUETAS_CAMPO[campo] ?? campo.replaceAll('_', ' ');
    }

    function enmascarar(valor: string): string {
        if (!valor) return '—';
        if (valor.length <= 4) return '••••';
        return `••••${valor.slice(-4)}`;
    }

    function valorAuditoria(campo: string, valor: any): string {
        if (valor === null || valor === undefined || valor === '') return '—';
        if (campo === 'monto') {
            return `$${Number(valor ?? 0).toLocaleString('es-MX', { minimumFractionDigits: 2 })}`;
        }
        if (campo === 'fecha') return fechaVisible(String(valor));
        if (campo === 'campos_extra') {
            if (!Array.isArray(valor) || valor.length === 0) return '—';
            return valor
                .map((item: any) => `${item?.nombre ?? 'Campo'}: ${item?.valor ?? ''}`)
                .join(' · ');
        }
        if (campo === 'curp' || campo === 'rfc' || campo === 'numero_cuenta') {
            return enmascarar(String(valor));
        }
        if (Array.isArray(valor)) return valor.join(', ');
        if (typeof valor === 'object') return JSON.stringify(valor);
        return String(valor);
    }

    function camposEvento(evento: any): any[] {
        return Array.isArray(evento?.detalle?.campos) ? evento.detalle.campos : [];
    }

    function detalleCamposDisponible(evento: any): boolean {
        return Array.isArray(evento?.detalle?.campos);
    }

    function etiquetaAccion(accion: string): string {
        const etiquetas: Record<string, string> = {
            'darfes.crear': 'Creación',
            'darfes.crear_canasta': 'Creación de canasta',
            'darfes.editar': 'Edición',
            'darfes.eliminar': 'Eliminación',
            'darfes.documento.subir': 'Documento cargado',
            'darfes.documento.eliminar': 'Documento eliminado',
            'darfes.folio.editar': 'Folio modificado',
            'darfes.formato': 'Recibo generado'
        };
        return etiquetas[accion] ?? accion.replaceAll('.', ' · ');
    }

    function resumenEvento(evento: any): string {
        const detalle = evento?.detalle ?? {};
        switch (evento?.accion) {
            case 'darfes.crear':
                return `DARFE ${detalle.numero_darfe ?? ''} creado`.trim();
            case 'darfes.crear_canasta':
                return `Canasta ${detalle.grupo ?? ''} creada`.trim();
            case 'darfes.editar': {
                const totalCampos = camposEvento(evento).length;
                return totalCampos === 0
                    ? 'Guardó sin cambios en los campos'
                    : `${totalCampos} campo${totalCampos === 1 ? '' : 's'} modificado${
                          totalCampos === 1 ? '' : 's'
                      }`;
            }
            case 'darfes.eliminar':
                return `DARFE ${detalle.numero_darfe ?? ''} eliminado`.trim();
            case 'darfes.documento.subir':
                return `Documento ${detalle.campo ?? ''} cargado`.trim();
            case 'darfes.documento.eliminar':
                return `Documento ${detalle.campo ?? ''} eliminado`.trim();
            case 'darfes.folio.editar':
                return detalle.anio
                    ? `Folio ${detalle.folio} del año ${detalle.anio}`
                    : `Folio ${detalle.folio ?? ''}`.trim();
            case 'darfes.formato':
                return detalle.folio
                    ? `Recibo generado con folio ${detalle.folio}`
                    : 'Recibo generado';
            default:
                return 'Evento registrado';
        }
    }

    function claseAccion(accion: string): string {
        if (accion === 'darfes.crear' || accion === 'darfes.crear_canasta') {
            return 'bg-emerald-500';
        }
        if (accion === 'darfes.eliminar' || accion === 'darfes.documento.eliminar') {
            return 'bg-rose-500';
        }
        if (accion === 'darfes.editar') return 'bg-amber-500';
        if (accion === 'darfes.folio.editar') return 'bg-violet-500';
        if (accion === 'darfes.formato') return 'bg-sky-500';
        return 'bg-slate-400';
    }

    function fechaHoraAuditoria(valor: string): string {
        return new Date(valor).toLocaleString('es-MX');
    }

    function agregarCampo() {
        form.campos_extra = [...form.campos_extra, { nombre: '', valor: '' }];
    }

    function quitarCampo(indice: number) {
        form.campos_extra = form.campos_extra.filter((_, i) => i !== indice);
    }

    function seleccionarArchivo(evento: Event, campo: string) {
        const entrada = evento.currentTarget as HTMLInputElement;
        archivos = { ...archivos, [campo]: entrada.files?.[0] ?? null };
    }

    function soltarArchivo(evento: DragEvent, campo: string) {
        evento.preventDefault();
        const archivo = evento.dataTransfer?.files?.[0];
        if (archivo) archivos = { ...archivos, [campo]: archivo };
    }

    function leerBase64(archivo: File): Promise<string> {
        return new Promise((resolve, reject) => {
            const lector = new FileReader();
            lector.onload = () => resolve(String(lector.result).split(',')[1] ?? '');
            lector.onerror = () => reject(lector.error);
            lector.readAsDataURL(archivo);
        });
    }

    async function persistir(): Promise<{ ids: string[]; depositoCoincide: boolean | null }> {
        const cuerpo = {
            numero_darfe: form.numero_darfe,
            fecha: form.fecha,
            nombre: form.nombre,
            expediente: normalizarExpediente(form.expediente),
            registro: form.registro,
            banco: form.banco,
            curp: form.curp,
            rfc: form.rfc,
            telefono: form.telefono,
            correo: form.correo,
            descripcion: form.descripcion,
            monto: Number(form.monto),
            numero_cuenta: form.numero_cuenta,
            tipo: form.tipo,
            estatus: form.estatus,
            campos_extra: form.campos_extra.filter(
                (campo) => campo.nombre.trim() !== '' || campo.valor.trim() !== ''
            )
        };
        let ids: string[] = [];
        if (editandoId) {
            const respuesta = await api<any>('PUT', `/api/v1/darfes/${editandoId}`, cuerpo);
            ids = [respuesta.id];
        } else {
            const respuesta = await api<any>('POST', '/api/v1/darfes', cuerpo);
            ids = Array.isArray(respuesta.darfes)
                ? respuesta.darfes.map((item: any) => item.id)
                : [respuesta.id];
        }

        let depositoCoincide: boolean | null = null;
        for (const id of ids) {
            for (const campo of CAMPOS_DOCUMENTO) {
                const archivo = archivos[campo.clave];
                if (!archivo) continue;
                const resultado = await api<any>('POST', `/api/v1/darfes/${id}/documentos`, {
                    campo: campo.clave,
                    nombre: archivo.name,
                    mime: archivo.type || 'application/octet-stream',
                    contenido_base64: await leerBase64(archivo)
                });
                if (campo.clave === 'DEPOSITO') depositoCoincide = resultado.coincide;
            }
        }
        return { ids, depositoCoincide };
    }

    async function guardar() {
        guardando = true;
        errorFormulario = '';
        try {
            const { ids, depositoCoincide } = await persistir();

            if (ids.length > 1) {
                app.avisar(`${ids.length} canastas creadas`);
            } else {
                app.avisar(editandoId ? 'DARFE actualizado' : 'DARFE creado');
            }
            if (depositoCoincide === true) {
                app.avisar('Depósito verificado: los datos coinciden');
            } else if (depositoCoincide === false) {
                app.avisar('Depósito cargado, pero los datos no coinciden', 'error');
            }
            modal = false;
            pagina = 1;
            await cargar();
        } catch (fallo) {
            errorFormulario = String(fallo);
        } finally {
            guardando = false;
        }
    }

    async function generarFormato() {
        guardando = true;
        errorFormulario = '';
        try {
            const esNuevo = !editandoId;
            let id = editandoId;
            if (!id) {
                const { ids } = await persistir();
                id = ids[0] ?? null;
            }
            if (!id) throw new Error('No se pudo guardar el DARFE para generar el formato');

            const ruta = await invoke<string>('guardar_formato_darfe', { id });
            app.avisar(`Formato DOCX guardado en ${ruta}`);
            if (esNuevo) {
                modal = false;
                pagina = 1;
                await cargar();
            }
        } catch (fallo) {
            errorFormulario = String(fallo);
        } finally {
            guardando = false;
        }
    }

    async function guardarFolio() {
        if (!editandoId) return;
        const folio = Number(folioFormulario);
        if (!Number.isInteger(folio) || folio <= 0) {
            errorFormulario = 'El folio debe ser un número positivo';
            return;
        }
        guardandoFolio = true;
        errorFormulario = '';
        try {
            const actualizado = await api<any>('PUT', `/api/v1/darfes/${editandoId}/folio`, {
                folio
            });
            folioFormulario = String(actualizado.folio_formato ?? folio);
            app.avisar(`Folio ${folio} asignado`);
            await cargar();
        } catch (fallo) {
            errorFormulario = String(fallo);
        } finally {
            guardandoFolio = false;
        }
    }

    async function eliminar(registro: any) {
        if (!confirm(`¿Eliminar el DARFE ${registro.numero_darfe}?`)) return;
        try {
            await api('DELETE', `/api/v1/darfes/${registro.id}`);
            app.avisar('DARFE eliminado');
            await cargar();
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    async function descargarDocumento(registro: any, campo: string) {
        try {
            const ruta = await invoke<string>('guardar_documento_darfe', {
                id: registro.id,
                campo
            });
            app.avisar(`Documento guardado en ${ruta}`);
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    async function verDocumento(registro: any, campo: string) {
        try {
            const documento = await invoke<{
                nombre: string;
                mime: string;
                contenido_base64: string;
            }>('ver_documento_darfe', { id: registro.id, campo });
            const binario = atob(documento.contenido_base64);
            const bytes = new Uint8Array(binario.length);
            for (let indice = 0; indice < binario.length; indice++) {
                bytes[indice] = binario.charCodeAt(indice);
            }
            const url = URL.createObjectURL(
                new Blob([bytes], { type: documento.mime || 'application/octet-stream' })
            );
            cerrarVista();
            vistaDocumento = {
                id: registro.id,
                campo,
                nombre: documento.nombre,
                mime: documento.mime,
                url
            };
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    function cerrarVista() {
        if (vistaDocumento) {
            URL.revokeObjectURL(vistaDocumento.url);
            vistaDocumento = null;
        }
    }

    async function exportar() {
        try {
            const consulta = new URLSearchParams();
            for (const [clave, valor] of Object.entries(filtros)) {
                if (String(valor).trim()) consulta.set(clave, String(valor).trim());
            }
            const ruta = await invoke<string>('guardar_exportacion_darfes', {
                consulta: consulta.toString() ? `?${consulta.toString()}` : ''
            });
            app.avisar(`Excel guardado en ${ruta}`);
        } catch (fallo) {
            app.avisar(String(fallo), 'error');
        }
    }

    function fechaVisible(fecha: string): string {
        return new Date(`${fecha}T00:00:00`).toLocaleDateString('es-MX');
    }

    function normalizarExpediente(valor: string): string {
        const texto = valor.trim();
        const indice = texto.indexOf('/');
        if (indice <= 0) return texto;
        const prefijo = texto.slice(0, indice);
        if (prefijo.length >= 8) return texto;
        return `${prefijo.padStart(8, '0')}${texto.slice(indice)}`;
    }

    async function consultarExpediente() {
        const numero = normalizarExpediente(form.expediente);
        if (!numero) {
            app.avisar('Capture el número de expediente', 'error');
            return;
        }
        form.expediente = numero;
        consultandoExpediente = true;
        avisoConsulta = '';
        try {
            const datos = await api<any>(
                'POST',
                '/api/v1/integraciones/expediente/consultar',
                { expediente: numero }
            );
            if (datos.nombre) form.nombre = datos.nombre;
            if (datos.correo) form.correo = datos.correo;
            if (datos.telefono) form.telefono = datos.telefono;
            if (datos.curp) form.curp = datos.curp;
            avisoConsulta = datos.curp
                ? 'Datos cargados desde la API'
                : 'Datos cargados; la API no devolvió CURP';
            app.avisar(avisoConsulta);
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            consultandoExpediente = false;
        }
    }

    async function importarPaperless() {
        if (!editandoId) return;
        const documento = paperlessId.trim();
        if (!documento) {
            app.avisar('Capture el ID del documento de Paperless', 'error');
            return;
        }
        importandoPaperless = true;
        try {
            const resultado = await api<any>(
                'POST',
                `/api/v1/darfes/${editandoId}/paperless`,
                { documento_id: documento, campo: paperlessCampo }
            );
            if (!documentosExistentes.includes(resultado.campo)) {
                documentosExistentes = [...documentosExistentes, resultado.campo];
            }
            paperlessId = '';
            if (resultado.campo === 'DEPOSITO') {
                app.avisar(
                    resultado.coincide
                        ? 'Depósito importado; los datos coinciden'
                        : 'Depósito importado; los datos no coinciden',
                    resultado.coincide ? 'ok' : 'error'
                );
            } else {
                app.avisar(`Documento importado en ${resultado.campo}`);
            }
        } catch (error) {
            app.avisar(String(error), 'error');
        } finally {
            importandoPaperless = false;
        }
    }

    function alternarColumna(clave: ClaveColumna, visible: boolean) {
        if (!visible && columnasVisibles === 1) return;
        columnas = { ...columnas, [clave]: visible };
        guardarColumnas();
    }

    function claveColumnas(): string {
        return `darfes.columnas.${app.usuario?.usuario_id ?? 'anonimo'}`;
    }

    function cargarColumnas() {
        try {
            const guardado = window.localStorage.getItem(claveColumnas());
            if (!guardado) return;
            const valores = JSON.parse(guardado) as Record<string, unknown>;
            const siguiente = { ...COLUMNAS_PREDETERMINADAS };
            for (const columna of COLUMNAS) {
                if (typeof valores?.[columna.clave] === 'boolean') {
                    siguiente[columna.clave] = valores[columna.clave] as boolean;
                }
            }
            if (COLUMNAS.some((columna) => siguiente[columna.clave])) columnas = siguiente;
        } catch {
            /* el equipo no tiene preferencias guardadas o no hay almacenamiento local */
        }
    }

    function guardarColumnas() {
        try {
            window.localStorage.setItem(claveColumnas(), JSON.stringify(columnas));
        } catch {
            /* si el almacenamiento local no está disponible se conserva la selección en memoria */
        }
    }

    function cerrarMenuColumnas(evento: MouseEvent) {
        if (!menuColumnas) return;
        const destino = evento.target as Node | null;
        if (destino && menuColumnasEl?.contains(destino)) return;
        menuColumnas = false;
    }

    function alPresionarTecla(evento: KeyboardEvent) {
        if (evento.key !== 'Escape') return;
        if (menuColumnas) {
            menuColumnas = false;
            return;
        }
        if (avanzado) avanzado = false;
    }

    function claseEstatus(estatus: string): string {
        if (estatus === 'COMPLETO') return 'bg-emerald-100 text-emerald-700';
        if (estatus === 'INCOMPLETO') return 'bg-amber-100 text-amber-700';
        return 'bg-red-100 text-red-700';
    }

    function pares(valor: any): Array<{ clave: string; total: number }> {
        if (!valor || typeof valor !== 'object') return [];
        return Object.entries(valor).map(([clave, total]) => ({ clave, total: Number(total) }));
    }
</script>

<svelte:window onclick={cerrarMenuColumnas} onkeydown={alPresionarTecla} />

<div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
            <p class="text-sm text-slate-500">
                {#if pestana === 'registros'}
                    {total} {total === 1 ? 'registro' : 'registros'} · registro, documentos y reportes
                {:else}
                    Reportes e indicadores
                {/if}
            </p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
            <div
                class="inline-flex rounded-lg bg-slate-100 p-0.5"
                role="tablist"
                aria-label="Vista de DARFES"
            >
                <button
                    class="rounded-md px-3 py-1.5 text-xs font-medium transition {pestana ===
                    'registros'
                        ? 'bg-white text-slate-800 shadow-sm'
                        : 'text-slate-500 hover:text-slate-700'}"
                    type="button"
                    role="tab"
                    aria-selected={pestana === 'registros'}
                    onclick={() => {
                        pestana = 'registros';
                        cargar();
                    }}
                >
                    Registros
                </button>
                <button
                    class="rounded-md px-3 py-1.5 text-xs font-medium transition {pestana ===
                    'reportes'
                        ? 'bg-white text-slate-800 shadow-sm'
                        : 'text-slate-500 hover:text-slate-700'}"
                    type="button"
                    role="tab"
                    aria-selected={pestana === 'reportes'}
                    onclick={() => {
                        pestana = 'reportes';
                        cargarReportes();
                    }}
                >
                    Reportes
                </button>
            </div>
            {#if pestana === 'registros' && app.permiso('darfes:crear')}
                <button class="btn-primario px-3 py-1.5 text-xs" type="button" onclick={abrirNuevo}>
                    <svg
                        class="h-3.5 w-3.5"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        aria-hidden="true"
                    >
                        <path d="M12 5v14M5 12h14" />
                    </svg>
                    Nuevo DARFE
                </button>
            {/if}
        </div>
    </div>

    {#if pestana === 'registros'}
        <section class="tarjeta overflow-hidden" aria-busy={cargando}>
            {#if cargando}
                <div class="h-0.5 w-full overflow-hidden bg-slate-100" aria-hidden="true">
                    <div class="h-full w-1/3 animate-pulse rounded-full bg-institucional-500"></div>
                </div>
            {/if}
            <div class="flex flex-wrap items-center gap-2 border-b border-slate-100 px-3 py-2">
                <div class="relative min-w-0 flex-1 sm:min-w-64">
                    <svg
                        class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        aria-hidden="true"
                    >
                        <circle cx="11" cy="11" r="7" />
                        <path d="m20 20-3.5-3.5" />
                    </svg>
                    <input
                        class="campo border-transparent bg-slate-50 pl-9 focus:bg-white"
                        placeholder="Buscar número, nombre, expediente, CURP, RFC, teléfono, correo o banco…"
                        aria-label="Buscar DARFES"
                        bind:value={filtros.buscar}
                        onkeydown={(evento) => evento.key === 'Enter' && buscar()}
                    />
                </div>
                <button
                    class="boton-icono"
                    type="button"
                    onclick={buscar}
                    disabled={cargando}
                    aria-label="Buscar"
                    title={cargando ? 'Buscando…' : 'Buscar'}
                >
                    <svg
                        class="h-4 w-4"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        aria-hidden="true"
                    >
                        <circle cx="11" cy="11" r="7" />
                        <path d="m20 20-3.5-3.5" />
                    </svg>
                </button>
                <div class="ml-auto flex items-center gap-1">
                    <button
                        class="boton-icono relative {avanzado ? 'boton-icono-activo' : ''}"
                        type="button"
                        onclick={() => (avanzado = !avanzado)}
                        aria-expanded={avanzado}
                        aria-label="Filtros"
                        title={avanzado ? 'Ocultar filtros' : 'Filtros'}
                    >
                        <svg
                            class="h-4 w-4"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            aria-hidden="true"
                        >
                            <path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h13M21 18h-1" />
                            <circle cx="16" cy="6" r="2" />
                            <circle cx="10" cy="12" r="2" />
                            <circle cx="19" cy="18" r="2" />
                        </svg>
                        {#if filtrosAvanzados > 0}
                            <span
                                class="absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full bg-institucional-500"
                                aria-hidden="true"
                            ></span>
                        {/if}
                    </button>
                    <div class="relative" bind:this={menuColumnasEl}>
                        <button
                            class="boton-icono {menuColumnas ? 'boton-icono-activo' : ''}"
                            type="button"
                            onclick={() => (menuColumnas = !menuColumnas)}
                            aria-expanded={menuColumnas}
                            aria-haspopup="true"
                            aria-label="Columnas visibles"
                            title="Columnas"
                        >
                            <svg
                                class="h-4 w-4"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                aria-hidden="true"
                            >
                                <rect x="3" y="4" width="7" height="16" rx="1" />
                                <rect x="14" y="4" width="7" height="16" rx="1" />
                            </svg>
                        </button>
                        {#if menuColumnas}
                            <div
                                class="absolute right-0 z-20 mt-1 w-64 rounded-lg border border-slate-200 bg-white p-3 shadow-xl"
                                role="group"
                                aria-label="Columnas visibles"
                            >
                                <p class="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
                                    Columnas visibles
                                </p>
                                <div class="space-y-1">
                                    {#each COLUMNAS as columna}
                                        <label class="flex items-center gap-2 text-sm text-slate-700">
                                            <input
                                                type="checkbox"
                                                checked={columnas[columna.clave]}
                                                disabled={columnas[columna.clave] && columnasVisibles === 1}
                                                onchange={(evento) =>
                                                    alternarColumna(
                                                        columna.clave,
                                                        evento.currentTarget.checked
                                                    )}
                                            />
                                            {columna.etiqueta}
                                        </label>
                                    {/each}
                                </div>
                            </div>
                        {/if}
                    </div>
                    {#if app.permiso('darfes:exportar')}
                        <button
                            class="boton-icono"
                            type="button"
                            onclick={exportar}
                            aria-label="Exportar a Excel"
                            title="Exportar a Excel"
                        >
                            <svg
                                class="h-4 w-4"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path d="M12 4v10" />
                                <path d="m8 10 4 4 4-4" />
                                <path d="M5 19h14" />
                            </svg>
                        </button>
                    {/if}
                </div>
            </div>

            {#if avanzado}
                <div class="grid gap-3 border-b border-slate-100 bg-slate-50/60 px-3 py-3 sm:grid-cols-2 lg:grid-cols-5">
                    <label class="block">
                        <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Fecha desde</span>
                        <input class="campo py-1.5 text-xs" type="date" bind:value={filtros.fecha_desde} />
                    </label>
                    <label class="block">
                        <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Fecha hasta</span>
                        <input class="campo py-1.5 text-xs" type="date" bind:value={filtros.fecha_hasta} />
                    </label>
                    <label class="block">
                        <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Estatus</span>
                        <select class="campo py-1.5 text-xs" bind:value={filtros.estatus}>
                            <option value="">Todos</option>
                            {#each ESTATUS as estatus}
                                <option value={estatus}>{estatus}</option>
                            {/each}
                        </select>
                    </label>
                    <label class="block">
                        <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Banco</span>
                        <input class="campo py-1.5 text-xs" bind:value={filtros.banco} />
                    </label>
                    <label class="block">
                        <span class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-500">Tipo</span>
                        <select class="campo py-1.5 text-xs" bind:value={filtros.tipo}>
                            <option value="">Todos</option>
                            {#each TIPOS as tipo}
                                <option value={tipo}>{tipo}</option>
                            {/each}
                        </select>
                    </label>
                    <div class="flex items-end gap-2 sm:col-span-2 lg:col-span-5">
                        <button class="btn-secundario px-3 py-1.5 text-xs" type="button" onclick={limpiarFiltros}>
                            Limpiar
                        </button>
                        <button class="btn-primario px-3 py-1.5 text-xs" type="button" onclick={buscar}>
                            Aplicar
                        </button>
                    </div>
                </div>
            {/if}

            <div class="max-h-[62vh] overflow-auto">
                <table class="tabla min-w-[1100px]">
                    <thead>
                        <tr>
                            {#if columnas.numero_darfe}<th>No. DARFE</th>{/if}
                            {#if columnas.fecha}<th>Fecha</th>{/if}
                            {#if columnas.nombre}<th>Nombre</th>{/if}
                            {#if columnas.expediente}<th>Número de expediente</th>{/if}
                            {#if columnas.descripcion}<th>Descripción</th>{/if}
                            {#if columnas.monto}<th>Monto</th>{/if}
                            {#if columnas.numero_cuenta}<th>Número de cuenta</th>{/if}
                            {#if columnas.tipo}<th>Tipo</th>{/if}
                            {#if columnas.banco}<th>Banco</th>{/if}
                            {#if columnas.estatus}<th>Estatus</th>{/if}
                            {#if columnas.documentos}<th>Documentos</th>{/if}
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each registros as registro (registro.id)}
                            <tr>
                                {#if columnas.numero_darfe}
                                    <td class="whitespace-nowrap text-xs leading-5">
                                        <span class="block font-mono font-medium text-institucional-600">
                                            {registro.numero_darfe}
                                        </span>
                                        <span class="block text-slate-500">
                                            Registro: {registro.registro || '—'}
                                        </span>
                                        <span class="block text-slate-500">
                                            CURP: {registro.curp || '—'}
                                        </span>
                                        <span class="block text-slate-500">
                                            RFC: {registro.rfc || '—'}
                                        </span>
                                    </td>
                                {/if}
                                {#if columnas.fecha}
                                    <td>{fechaVisible(registro.fecha)}</td>
                                {/if}
                                {#if columnas.nombre}
                                    <td>{registro.nombre}</td>
                                {/if}
                                {#if columnas.expediente}
                                    <td class="whitespace-nowrap text-xs text-slate-700">
                                        {registro.expediente || '—'}
                                    </td>
                                {/if}
                                {#if columnas.descripcion}
                                    <td>{registro.descripcion || '—'}</td>
                                {/if}
                                {#if columnas.monto}
                                    <td>
                                        ${Number(registro.monto ?? 0).toLocaleString('es-MX', {
                                            minimumFractionDigits: 2
                                        })}
                                    </td>
                                {/if}
                                {#if columnas.numero_cuenta}
                                    <td class="whitespace-nowrap font-mono text-xs">
                                        {registro.numero_cuenta || '—'}
                                    </td>
                                {/if}
                                {#if columnas.tipo}
                                    <td>{registro.tipo}</td>
                                {/if}
                                {#if columnas.banco}
                                    <td>{registro.banco || '—'}</td>
                                {/if}
                                {#if columnas.estatus}
                                    <td>
                                        <span class="insignia {claseEstatus(registro.estatus)}">
                                            {registro.estatus}
                                        </span>
                                    </td>
                                {/if}
                                {#if columnas.documentos}
                                    <td>
                                        <div class="flex flex-wrap gap-1">
                                            {#each CAMPOS_DOCUMENTO as campo}
                                                {#if registro.documentos?.includes(campo.clave)}
                                                    <button
                                                        class="text-xs underline text-institucional-600"
                                                        type="button"
                                                        title={`Ver ${campo.etiqueta}`}
                                                        onclick={() => verDocumento(registro, campo.clave)}
                                                    >
                                                        {campo.etiqueta}
                                                    </button>
                                                {/if}
                                            {/each}
                                            {#if registro.documentos?.includes('DEPOSITO')}
                                                <span
                                                    class="insignia {registro.deposito_coincide === true
                                                        ? 'bg-emerald-100 text-emerald-700'
                                                        : registro.deposito_coincide === false
                                                          ? 'bg-red-100 text-red-700'
                                                          : 'bg-slate-100 text-slate-600'}"
                                                    title={registro.deposito_coincide === true
                                                        ? 'Cuenta, monto y nombre del depósito coinciden'
                                                        : registro.deposito_coincide === false
                                                          ? 'Los datos del depósito no coinciden con lo capturado'
                                                          : 'Depósito cargado sin validación'}
                                                >
                                                    Depósito {registro.deposito_coincide === true
                                                        ? '✓'
                                                        : registro.deposito_coincide === false
                                                          ? '✗'
                                                          : '·'}
                                                </span>
                                            {/if}
                                        </div>
                                    </td>
                                {/if}
                                <td class="whitespace-nowrap text-right">
                                    <div class="flex justify-end gap-1">
                                        {#if app.esAdministrador()}
                                            <button
                                                class="boton-icono h-7 w-7"
                                                type="button"
                                                onclick={() => abrirAuditoria(registro)}
                                                aria-label={`Ver auditoría del DARFE ${registro.numero_darfe}`}
                                                title="Auditoría"
                                            >
                                                <svg
                                                    class="h-4 w-4"
                                                    viewBox="0 0 24 24"
                                                    fill="none"
                                                    stroke="currentColor"
                                                    stroke-width="2"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"
                                                    aria-hidden="true"
                                                >
                                                    <path d="M3 12a9 9 0 1 0 3-6.7" />
                                                    <path d="M3 4v5h5" />
                                                    <path d="M12 7v5l3 2" />
                                                </svg>
                                            </button>
                                        {/if}
                                        {#if app.permiso('darfes:editar')}
                                            <button
                                                class="boton-icono h-7 w-7"
                                                type="button"
                                                onclick={() => abrirEdicion(registro)}
                                                aria-label={`Editar DARFE ${registro.numero_darfe}`}
                                                title="Editar"
                                            >
                                                <svg
                                                    class="h-4 w-4"
                                                    viewBox="0 0 24 24"
                                                    fill="none"
                                                    stroke="currentColor"
                                                    stroke-width="2"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"
                                                    aria-hidden="true"
                                                >
                                                    <path d="M12 20h9" />
                                                    <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
                                                </svg>
                                            </button>
                                        {/if}
                                        {#if app.permiso('darfes:eliminar')}
                                            <button
                                                class="boton-icono h-7 w-7 hover:bg-rose-50 hover:text-rose-600"
                                                type="button"
                                                onclick={() => eliminar(registro)}
                                                aria-label={`Eliminar DARFE ${registro.numero_darfe}`}
                                                title="Eliminar"
                                            >
                                                <svg
                                                    class="h-4 w-4"
                                                    viewBox="0 0 24 24"
                                                    fill="none"
                                                    stroke="currentColor"
                                                    stroke-width="2"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"
                                                    aria-hidden="true"
                                                >
                                                    <path d="M4 7h16" />
                                                    <path d="M9 7V4h6v3" />
                                                    <path d="M6 7l1 13h10l1-13" />
                                                    <path d="M10 11v5M14 11v5" />
                                                </svg>
                                            </button>
                                        {/if}
                                    </div>
                                </td>
                            </tr>
                        {:else}
                            <tr>
                                <td colspan={columnasVisibles + 1} class="py-8 text-center text-sm text-slate-500">
                                    {#if cargando}
                                        <span class="inline-flex items-center gap-2">
                                            <svg
                                                class="h-4 w-4 animate-spin text-institucional-500"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                                aria-hidden="true"
                                            >
                                                <path d="M21 12a9 9 0 1 1-6.22-8.56" />
                                            </svg>
                                            Buscando…
                                        </span>
                                    {:else}
                                        No hay DARFES con esos filtros.
                                    {/if}
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
            <Paginacion
                {pagina}
                {paginas}
                total={total}
                onCambiar={(valor) => {
                    pagina = valor;
                    cargar();
                }}
            />
        </section>
    {:else}
        <section class="grid grid-cols-2 gap-4 lg:grid-cols-4">
            <div class="tarjeta p-4">
                <p class="text-xs uppercase tracking-wide text-slate-500">Total</p>
                <p class="mt-1 text-2xl font-semibold text-institucional-600">{reportes?.total ?? 0}</p>
            </div>
            {#each pares(reportes?.por_estatus) as item}
                <div class="tarjeta p-4">
                    <p class="text-xs uppercase tracking-wide text-slate-500">{item.clave}</p>
                    <p class="mt-1 text-2xl font-semibold text-institucional-600">{item.total}</p>
                </div>
            {/each}
        </section>

        <div class="grid gap-4 lg:grid-cols-2">
            <section class="tarjeta p-4">
                <h3 class="mb-3 text-sm font-semibold text-slate-700">Por banco</h3>
                {#each reportes?.por_banco ?? [] as item}
                    <div class="mb-2 flex items-center justify-between gap-3 text-sm">
                        <span class="truncate text-slate-600">{item.clave}</span>
                        <span class="font-semibold text-slate-800">{item.total}</span>
                    </div>
                {:else}
                    <p class="text-sm text-slate-500">Sin datos.</p>
                {/each}
            </section>

            <section class="tarjeta p-4">
                <h3 class="mb-3 text-sm font-semibold text-slate-700">Documentos cargados</h3>
                {#each pares(reportes?.documentos) as item}
                    <div class="mb-2 flex items-center justify-between gap-3 text-sm">
                        <span class="text-slate-600">{item.clave.replaceAll('_', ' ')}</span>
                        <span class="font-semibold text-slate-800">{item.total}</span>
                    </div>
                {:else}
                    <p class="text-sm text-slate-500">Sin documentos.</p>
                {/each}
            </section>
        </div>

        <section class="tarjeta p-4">
            <h3 class="mb-3 text-sm font-semibold text-slate-700">Últimos DARFES</h3>
            <ul class="space-y-2 text-sm">
                {#each reportes?.recientes ?? [] as item}
                    <li class="flex items-center justify-between gap-3 border-b border-slate-100 pb-2">
                        <span class="font-mono text-xs text-institucional-600">{item.numero_darfe}</span>
                        <span class="flex-1 truncate text-slate-600">{item.nombre}</span>
                        <span class="text-slate-500">{fechaVisible(item.fecha)}</span>
                        <span class="insignia {claseEstatus(item.estatus)}">{item.estatus}</span>
                    </li>
                {:else}
                    <li class="text-slate-500">Sin registros.</li>
                {/each}
            </ul>
        </section>
    {/if}
</div>

{#if modal}
    <Modal
        titulo={editandoId ? 'Editar DARFE' : 'Nuevo DARFE'}
        ancho={85}
        oncerrar={() => (modal = false)}
    >
        <form
            class="space-y-4"
            onsubmit={(evento) => {
                evento.preventDefault();
                guardar();
            }}
        >
            <div
                class="flex flex-wrap gap-1 rounded-xl bg-slate-100 p-1"
                role="tablist"
                aria-label="Secciones del formulario DARFE"
            >
                {#each PESTANAS_FORM as pestana}
                    <button
                        class="rounded-lg px-3 py-2 text-sm font-medium transition {pestanaForm ===
                        pestana.clave
                            ? 'bg-white text-institucional-700 shadow-sm'
                            : 'text-slate-600 hover:bg-white/60'}"
                        type="button"
                        role="tab"
                        aria-selected={pestanaForm === pestana.clave}
                        onclick={() => (pestanaForm = pestana.clave)}
                    >
                        {pestana.etiqueta}
                    </button>
                {/each}
            </div>

            {#if pestanaForm === 'generales'}
                <section class="rounded-lg border border-slate-200 bg-slate-50/60 p-4">
                <h3 class="mb-3 text-sm font-semibold text-slate-700">Identificación del DARFE</h3>
                <div class="grid gap-3 sm:grid-cols-2">
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">No. de DARFE</span>
                        <input class="campo font-mono" maxlength="100" bind:value={form.numero_darfe} required />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Fecha</span>
                        <input class="campo" type="date" bind:value={form.fecha} required />
                    </label>
                    <label class="block text-sm sm:col-span-2">
                        <span class="mb-1 block font-medium text-slate-600">Nombre</span>
                        <input class="campo" maxlength="200" bind:value={form.nombre} required />
                    </label>
                    <label class="block text-sm sm:col-span-2">
                        <span class="mb-1 block font-medium text-slate-600">Descripción</span>
                        <textarea
                            class="campo h-20"
                            maxlength="500"
                            bind:value={form.descripcion}
                            disabled={form.tipo === 'CANASTA'}
                            placeholder={form.tipo === 'CANASTA'
                                ? 'Se generará automáticamente para cada canasta'
                                : 'Descripción del DARFE'}
                        ></textarea>
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Tipo</span>
                        <select class="campo" bind:value={form.tipo} disabled={Boolean(editandoId)}>
                            {#each TIPOS as tipo}
                                <option value={tipo}>{tipo}</option>
                            {/each}
                        </select>
                        {#if form.tipo === 'CANASTA' && !editandoId}
                            <span class="mt-1 block text-xs text-slate-500">
                                Se crearán 3 registros mensuales automáticamente.
                            </span>
                        {/if}
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Estatus</span>
                        <select class="campo" bind:value={form.estatus}>
                            {#each ESTATUS as estatus}
                                <option value={estatus}>{estatus}</option>
                            {/each}
                        </select>
                    </label>
                </div>
            </section>

            {#if editandoId && app.permiso('darfes:folio')}
                <section class="rounded-lg border border-slate-200 bg-slate-50/60 p-4">
                    <h3 class="mb-3 text-sm font-semibold text-slate-700">Folio del recibo</h3>
                    <div class="flex flex-wrap items-end gap-2">
                        <label class="block text-sm">
                            <span class="mb-1 block font-medium text-slate-600">Folio</span>
                            <input
                                class="campo w-40"
                                type="number"
                                min="1"
                                step="1"
                                bind:value={folioFormulario}
                            />
                        </label>
                        <button
                            class="btn-secundario"
                            type="button"
                            onclick={guardarFolio}
                            disabled={guardandoFolio || guardando}
                        >
                            {guardandoFolio ? 'Guardando…' : 'Guardar folio'}
                        </button>
                        <span class="text-xs text-slate-500">
                            Solo el administrador puede cambiar el folio asignado.
                        </span>
                    </div>
                </section>
            {/if}
            {/if}

            {#if pestanaForm === 'pago'}
            <section class="rounded-lg border border-slate-200 bg-slate-50/60 p-4">
                <h3 class="mb-3 text-sm font-semibold text-slate-700">Datos de pago</h3>
                <div class="grid gap-3 sm:grid-cols-2">
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Monto</span>
                        <input class="campo" type="number" min="0" step="0.01" bind:value={form.monto} required />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Número de cuenta</span>
                        <input class="campo" maxlength="50" bind:value={form.numero_cuenta} />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Banco</span>
                        <input class="campo" bind:value={form.banco} />
                    </label>
                </div>
            </section>
            {/if}

            {#if pestanaForm === 'contacto'}
            <section class="rounded-lg border border-slate-200 bg-slate-50/60 p-4">
                <h3 class="mb-3 text-sm font-semibold text-slate-700">Datos personales y contacto</h3>
                <div class="grid gap-3 sm:grid-cols-2">
                    <label class="block text-sm sm:col-span-2">
                        <span class="mb-1 block font-medium text-slate-600">Número de expediente</span>
                        <input
                            class="campo"
                            maxlength="200"
                            bind:value={form.expediente}
                            onblur={() => (form.expediente = normalizarExpediente(form.expediente))}
                        />
                        <div class="mt-1 flex flex-wrap items-center gap-2">
                            <button
                                class="btn-secundario px-2 py-1 text-xs"
                                type="button"
                                onclick={consultarExpediente}
                                disabled={consultandoExpediente}
                            >
                                {consultandoExpediente ? 'Consultando…' : 'Consultar expediente'}
                            </button>
                            {#if avisoConsulta}
                                <span class="text-xs text-slate-500">{avisoConsulta}</span>
                            {/if}
                        </div>
                    </label>
                    <label class="block text-sm sm:col-span-2">
                        <span class="mb-1 block font-medium text-slate-600">Número de registro</span>
                        <input class="campo" maxlength="100" bind:value={form.registro} />
                    </label>
                    <label class="block text-sm sm:col-span-2">
                        <span class="mb-1 block font-medium text-slate-600">CURP</span>
                        <input class="campo font-mono uppercase" maxlength="18" bind:value={form.curp} />
                    </label>
                    <label class="block text-sm sm:col-span-2">
                        <span class="mb-1 block font-medium text-slate-600">RFC</span>
                        <input class="campo font-mono uppercase" maxlength="13" bind:value={form.rfc} />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Teléfono</span>
                        <input
                            class="campo"
                            type="tel"
                            maxlength="30"
                            bind:value={form.telefono}
                            placeholder="55 5555 5555"
                        />
                    </label>
                    <label class="block text-sm">
                        <span class="mb-1 block font-medium text-slate-600">Correo</span>
                        <input
                            class="campo"
                            type="email"
                            maxlength="200"
                            bind:value={form.correo}
                            placeholder="persona@ejemplo.mx"
                        />
                    </label>
                </div>
            </section>
            {/if}

            {#if pestanaForm === 'documentos'}
            <section class="space-y-2 rounded-lg border border-slate-200 bg-slate-50/60 p-4">
                <div class="flex items-center justify-between">
                    <h3 class="text-sm font-semibold text-slate-700">Documentos</h3>
                    {#if editandoId}
                        <span class="text-xs text-slate-500">Los archivos nuevos reemplazan al existente.</span>
                    {/if}
                </div>

                {#if editandoId}
                    <div class="rounded-lg border border-slate-200 bg-white p-3">
                        <p class="text-sm font-medium text-slate-700">Importar desde Paperless-ngx</p>
                        <div class="mt-2 flex flex-wrap items-end gap-2">
                            <label class="block text-sm">
                                <span class="mb-1 block text-xs text-slate-500">ID del documento</span>
                                <input
                                    class="campo w-40"
                                    inputmode="numeric"
                                    bind:value={paperlessId}
                                    placeholder="123"
                                />
                            </label>
                            <label class="block text-sm">
                                <span class="mb-1 block text-xs text-slate-500">Campo destino</span>
                                <select class="campo w-48" bind:value={paperlessCampo}>
                                    {#each CAMPOS_DOCUMENTO as campo}
                                        <option value={campo.clave}>{campo.etiqueta}</option>
                                    {/each}
                                </select>
                            </label>
                            <button
                                class="btn-secundario"
                                type="button"
                                onclick={importarPaperless}
                                disabled={importandoPaperless}
                            >
                                {importandoPaperless ? 'Importando…' : 'Importar'}
                            </button>
                        </div>
                    </div>
                {:else}
                    <p class="rounded-lg bg-slate-50 px-3 py-2 text-xs text-slate-500">
                        Guarde el DARFE para importar documentos desde Paperless-ngx.
                    </p>
                {/if}
                <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                    {#each CAMPOS_DOCUMENTO as campo}
                        <div
                            class="rounded-lg border border-dashed border-slate-300 bg-white p-3 text-sm"
                            role="group"
                            ondragover={(evento) => evento.preventDefault()}
                            ondrop={(evento) => soltarArchivo(evento, campo.clave)}
                        >
                            <p class="font-medium text-slate-700">{campo.etiqueta}</p>
                            <p class="text-xs text-slate-500">Arrastra el archivo aquí o selecciónalo.</p>
                            {#if documentosExistentes.includes(campo.clave)}
                                <button
                                    class="mt-1 text-xs underline text-institucional-600"
                                    type="button"
                                    onclick={() => verDocumento({ id: editandoId }, campo.clave)}
                                >
                                    Ver actual
                                </button>
                            {/if}
                            <input
                                class="campo mt-2 text-xs"
                                type="file"
                                accept="application/pdf,image/*"
                                onchange={(evento) => seleccionarArchivo(evento, campo.clave)}
                            />
                            {#if archivos[campo.clave]}
                                <p class="mt-1 truncate text-xs text-emerald-700">
                                    {archivos[campo.clave]?.name}
                                </p>
                            {/if}
                        </div>
                    {/each}
                </div>
            </section>

            {/if}

            {#if pestanaForm === 'campos'}
            <section class="space-y-2 rounded-lg border border-slate-200 bg-slate-50/60 p-4">
                <div class="flex items-center justify-between">
                    <h3 class="text-sm font-semibold text-slate-700">Campos adicionales</h3>
                    <button class="btn-secundario" type="button" onclick={agregarCampo}>Agregar campo</button>
                </div>
                {#each form.campos_extra as campo, indice}
                    <div class="flex gap-2">
                        <input class="campo" placeholder="Nombre del campo" bind:value={campo.nombre} />
                        <input class="campo" placeholder="Valor" bind:value={campo.valor} />
                        <button class="btn-secundario" type="button" onclick={() => quitarCampo(indice)}>
                            Quitar
                        </button>
                    </div>
                {/each}
            </section>
            {/if}

            {#if errorFormulario}
                <p class="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700">{errorFormulario}</p>
            {/if}

            <div class="flex flex-wrap justify-end gap-2">
                {#if editandoId && app.permiso('darfes:duplicar')}
                    <button
                        class="btn-secundario"
                        type="button"
                        onclick={duplicar}
                        disabled={guardando}
                        title="Crea un nuevo DARFE con los mismos datos; el número se captura de nuevo"
                    >
                        Duplicar
                    </button>
                {/if}
                <button
                    class="btn-secundario"
                    type="button"
                    onclick={generarFormato}
                    disabled={guardando}
                    title="Asigna el folio anual y descarga el recibo en Word"
                >
                    Descargar formato DOCX
                </button>
                <button class="btn-secundario" type="button" onclick={() => (modal = false)} disabled={guardando}>
                    Cancelar
                </button>
                <button class="btn-primario" type="submit" disabled={guardando}>
                    {guardando ? 'Guardando…' : 'Guardar'}
                </button>
            </div>
        </form>
    </Modal>
{/if}

{#if vistaDocumento}
    <Modal
        titulo={`Documento: ${vistaDocumento.nombre}`}
        ancho={80}
        oncerrar={cerrarVista}
    >
        {#if vistaDocumento.mime.startsWith('image/')}
            <img
                src={vistaDocumento.url}
                alt={vistaDocumento.nombre}
                class="mx-auto max-h-[70vh] max-w-full object-contain"
            />
        {:else if vistaDocumento.mime === 'application/pdf'}
            <iframe
                src={vistaDocumento.url}
                title={vistaDocumento.nombre}
                class="h-[70vh] w-full rounded-lg border border-slate-200"
            ></iframe>
        {:else}
            <p class="rounded-lg bg-slate-50 px-3 py-4 text-sm text-slate-600">
                Este tipo de archivo no se puede previsualizar. Descárgalo para abrirlo con el
                programa correspondiente.
            </p>
        {/if}

        <div class="mt-4 flex flex-wrap justify-end gap-2">
            <button
                class="btn-secundario"
                type="button"
                onclick={() => descargarDocumento({ id: vistaDocumento!.id }, vistaDocumento!.campo)}
            >
                Descargar
            </button>
            <button class="btn-primario" type="button" onclick={cerrarVista}>Cerrar</button>
        </div>
    </Modal>
{/if}

{#if auditoriaAbierta}
    <Modal
        titulo={`Auditoría · ${auditoriaDarfe?.numero_darfe ?? 'DARFE'}`}
        ancho={80}
        oncerrar={cerrarAuditoria}
    >
        <div class="space-y-4">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <p class="text-sm text-slate-600">
                        Historial completo de cambios del DARFE. Solo el Administrador puede consultarlo.
                    </p>
                    <p class="mt-1 text-xs text-slate-500">
                        CURP, RFC y número de cuenta se muestran parcialmente enmascarados.
                    </p>
                </div>
                <span class="insignia bg-slate-100 text-slate-600">
                    {auditoriaEventos.length} evento{auditoriaEventos.length === 1 ? '' : 's'}
                </span>
            </div>

            {#if auditoriaCargando}
                <div class="space-y-2" aria-label="Cargando auditoría">
                    {#each Array(3) as _}
                        <div class="h-20 animate-pulse rounded-lg bg-slate-100"></div>
                    {/each}
                </div>
            {:else if auditoriaError}
                <p class="rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700">{auditoriaError}</p>
            {:else if auditoriaEventos.length === 0}
                <p class="rounded-lg bg-slate-50 px-3 py-6 text-center text-sm text-slate-500">
                    Este DARFE todavía no tiene eventos de auditoría.
                </p>
            {:else}
                <ol class="relative space-y-3 border-l border-slate-200 pl-5">
                    {#each auditoriaEventos as evento (evento.id)}
                        <li class="relative">
                            <span
                                class="absolute -left-[26px] top-2 h-2.5 w-2.5 rounded-full ring-4 ring-white {claseAccion(
                                    evento.accion
                                )}"
                                aria-hidden="true"
                            ></span>
                            <div class="rounded-lg border border-slate-200 bg-white p-3">
                                <div class="flex flex-wrap items-start justify-between gap-2">
                                    <div class="min-w-0">
                                        <p class="text-sm font-medium text-slate-800">
                                            {etiquetaAccion(evento.accion)}
                                        </p>
                                        <p class="mt-0.5 text-xs text-slate-500">
                                            {resumenEvento(evento)}
                                        </p>
                                    </div>
                                    <time
                                        class="whitespace-nowrap text-xs text-slate-500"
                                        datetime={evento.ocurrido_en}
                                    >
                                        {fechaHoraAuditoria(evento.ocurrido_en)}
                                    </time>
                                </div>

                                <div class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-slate-500">
                                    <span class="font-medium text-slate-700">
                                        {evento.actor?.nombre ?? 'Sistema'}
                                    </span>
                                    {#if evento.actor?.correo}
                                        <span>· {evento.actor.correo}</span>
                                    {/if}
                                    {#if evento.ip}
                                        <span class="text-slate-400">· IP {evento.ip}</span>
                                    {/if}
                                    {#if evento.resultado !== 'EXITO'}
                                        <span class="insignia bg-rose-100 text-rose-700">
                                            {evento.resultado}
                                        </span>
                                    {/if}
                                </div>

                                {#if camposEvento(evento).length > 0}
                                    <div class="mt-2 overflow-x-auto rounded-md border border-slate-100">
                                        <table class="w-full text-xs">
                                            <thead class="bg-slate-50 text-left text-slate-500">
                                                <tr>
                                                    <th class="px-2 py-1.5 font-medium">Campo</th>
                                                    <th class="px-2 py-1.5 font-medium">Antes</th>
                                                    <th class="px-2 py-1.5 font-medium">Después</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {#each camposEvento(evento) as cambio}
                                                    <tr class="border-t border-slate-100 align-top">
                                                        <td class="px-2 py-1.5 font-medium text-slate-700">
                                                            {etiquetaCampo(cambio.campo)}
                                                        </td>
                                                        <td class="max-w-64 px-2 py-1.5 text-slate-500">
                                                            {valorAuditoria(cambio.campo, cambio.anterior)}
                                                        </td>
                                                        <td class="max-w-64 px-2 py-1.5 text-slate-700">
                                                            {valorAuditoria(cambio.campo, cambio.nuevo)}
                                                        </td>
                                                    </tr>
                                                {/each}
                                            </tbody>
                                        </table>
                                    </div>
                                {:else if evento.accion === 'darfes.editar'}
                                    {#if detalleCamposDisponible(evento)}
                                        <p class="mt-2 rounded-md bg-slate-50 px-2 py-1.5 text-xs text-slate-500">
                                            No se modificó ningún campo.
                                        </p>
                                    {:else}
                                        <p class="mt-2 rounded-md bg-slate-50 px-2 py-1.5 text-xs text-slate-500">
                                            La edición se registró antes de habilitar el detalle por campo.
                                        </p>
                                    {/if}
                                {/if}
                            </div>
                        </li>
                    {/each}
                </ol>
            {/if}

            <div class="flex justify-end">
                <button class="btn-primario" type="button" onclick={cerrarAuditoria}>Cerrar</button>
            </div>
        </div>
    </Modal>
{/if}
