-- Tipos de apoyo del DARFE y fecha congelada del recibo.
ALTER TABLE darfes DROP CONSTRAINT IF EXISTS darfes_tipo_check;
ALTER TABLE darfes ADD CONSTRAINT darfes_tipo_check CHECK (tipo IN (
    'CANASTA',
    'REEMBOLSO',
    'APOYO PARA VIÁTICOS',
    'MEDICAMENTO',
    'OTRAS AYUDAS',
    'VIATICOS',
    'AYUDAS EN MATERIA DE SALUD',
    'GASTOS DE TRASLADO, HOSPEDAJE (VIATICOS)'
));

-- El número es único para todos los tipos salvo CANASTA, cuyos tres
-- registros comparten el mismo número.
DROP INDEX IF EXISTS darfes_numero_reembolso;
CREATE UNIQUE INDEX darfes_numero_unico
    ON darfes (numero_darfe)
    WHERE eliminado_en IS NULL AND tipo <> 'CANASTA';

-- Fecha con la que se emitió el recibo: una reimpresión conserva mes y año.
ALTER TABLE darfes ADD COLUMN formato_fecha date;
