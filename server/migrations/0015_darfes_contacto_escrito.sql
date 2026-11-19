-- Datos de contacto y documento de escrito libre.
ALTER TABLE darfes
    ADD COLUMN telefono text NOT NULL DEFAULT '',
    ADD COLUMN correo text NOT NULL DEFAULT '';

ALTER TABLE darfe_documentos DROP CONSTRAINT IF EXISTS darfe_documentos_campo_check;
ALTER TABLE darfe_documentos ADD CONSTRAINT darfe_documentos_campo_check
    CHECK (campo IN (
        'CURP',
        'RFC',
        'REGISTRO',
        'IDENTIFICACION',
        'ESTADO_CUENTA',
        'DEPOSITO',
        'ESCRITO_LIBRE'
    ));
