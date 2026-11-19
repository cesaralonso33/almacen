-- Folio consecutivo del recibo PDF, con reinicio anual.
CREATE TABLE darfe_folios (
    anio integer PRIMARY KEY,
    ultimo integer NOT NULL CHECK (ultimo >= 557)
);

-- El folio se asigna una sola vez por DARFE para que una reimpresión no
-- consuma un número nuevo.
ALTER TABLE darfes
    ADD COLUMN folio_formato integer,
    ADD COLUMN folio_anio integer;

CREATE UNIQUE INDEX darfes_folio_formato
    ON darfes (folio_anio, folio_formato)
    WHERE folio_formato IS NOT NULL;
