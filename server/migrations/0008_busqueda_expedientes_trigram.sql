-- 0008: acelerar búsquedas parciales con ILIKE '%texto%'.
-- pg_trgm forma parte de PostgreSQL contrib; si el paquete no está instalado,
-- la migración falla explícitamente y la API no arranca con un esquema parcial.
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX expedientes_numero_trgm_idx
    ON expedientes USING gin (numero gin_trgm_ops);

CREATE INDEX expedientes_descripcion_trgm_idx
    ON expedientes USING gin (descripcion gin_trgm_ops);
