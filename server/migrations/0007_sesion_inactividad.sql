-- 0007: cierre de sesión por inactividad y duración de 18 horas.
--
-- La duración absoluta se controla con sesiones.expira_en (configurable por entorno).
-- La columna nueva registra la última petición del usuario para aplicar el tiempo de espera.

ALTER TABLE sesiones ADD COLUMN ultima_actividad_en timestamptz NOT NULL DEFAULT now();

CREATE INDEX sesiones_por_actividad ON sesiones (ultima_actividad_en)
    WHERE revocada_en IS NULL;
