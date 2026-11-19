-- La tabla de DARFES se presenta por fecha de creación (más reciente primero).
CREATE INDEX darfes_creado_en
    ON darfes (creado_en DESC, id DESC)
    WHERE eliminado_en IS NULL;
