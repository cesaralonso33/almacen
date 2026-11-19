-- Ampliación de DARFES: descripción, monto, cuenta, tipo y canastas.
ALTER TABLE darfes
    ADD COLUMN descripcion text NOT NULL DEFAULT '',
    ADD COLUMN monto numeric(14, 2) NOT NULL DEFAULT 0 CHECK (monto >= 0),
    ADD COLUMN numero_cuenta text NOT NULL DEFAULT '',
    ADD COLUMN tipo text NOT NULL DEFAULT 'REEMBOLSO'
        CHECK (tipo IN ('CANASTA', 'REEMBOLSO')),
    ADD COLUMN grupo_canasta uuid,
    ADD COLUMN item_canasta smallint CHECK (item_canasta BETWEEN 1 AND 3);

-- El número se conserva único para reembolsos. Las tres canastas comparten
-- número y se distinguen por grupo/item.
ALTER TABLE darfes DROP CONSTRAINT IF EXISTS darfes_numero_darfe_key;
CREATE UNIQUE INDEX darfes_numero_reembolso
    ON darfes (numero_darfe)
    WHERE eliminado_en IS NULL AND tipo = 'REEMBOLSO';
CREATE UNIQUE INDEX darfes_canasta_grupo
    ON darfes (grupo_canasta, item_canasta)
    WHERE grupo_canasta IS NOT NULL;
CREATE INDEX darfes_por_tipo ON darfes (tipo, fecha DESC) WHERE eliminado_en IS NULL;
CREATE INDEX darfes_descripcion_trgm ON darfes USING gin (descripcion gin_trgm_ops)
    WHERE eliminado_en IS NULL;

ALTER TABLE darfe_documentos
    ADD COLUMN datos_extraidos jsonb,
    ADD COLUMN coincide boolean;

ALTER TABLE darfe_documentos DROP CONSTRAINT IF EXISTS darfe_documentos_campo_check;
ALTER TABLE darfe_documentos ADD CONSTRAINT darfe_documentos_campo_check
    CHECK (campo IN ('CURP', 'RFC', 'REGISTRO', 'IDENTIFICACION', 'ESTADO_CUENTA', 'DEPOSITO'));
