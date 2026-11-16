-- 0006: el número de expediente es único en todo el sistema.
--
-- Antes de crear el índice se verifica que no existan duplicados; si los hay,
-- la migración falla con un mensaje claro en lugar de romper en silencio.

DO $$
DECLARE
    duplicados integer;
BEGIN
    SELECT count(*) INTO duplicados
    FROM (SELECT numero FROM expedientes GROUP BY numero HAVING count(*) > 1) AS repetidos;

    IF duplicados > 0 THEN
        RAISE EXCEPTION 'Existen % números de expediente duplicados; resuélvalos antes de aplicar esta migración', duplicados;
    END IF;
END $$;

DROP INDEX IF EXISTS expedientes_numero_unico;

CREATE UNIQUE INDEX expedientes_numero_global ON expedientes (numero);
