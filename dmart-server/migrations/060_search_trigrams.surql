-- SPEC-052: soporte de búsqueda parcial de pacientes sin exponer la PHI.
--
-- El problema: la búsqueda por índice ciego sólo resolvía el **valor exacto**
-- de `nombre`, `historia_clinica` y `cedula`. Escribir "gust" no encontraba a
-- nadie, y el apellido no estaba indexado en absoluto, así que la mitad de las
-- búsquedas reales ("Ortiz") devolvían cero.
--
-- Por qué trigramas y no un índice de texto en claro (Tantivy/Lucene): la PHI
-- está cifrada en reposo por requisito de SPEC-052, y un índice invertido con
-- los nombres en claro escribiría la PHI sin cifrar en disco, anulando la
-- garantía. Un índice de trigramas **ciegos** (HMAC-SHA256 de cada trigrama,
-- con la misma clave de índice que el resto) mantiene la PHI fuera del disco:
-- sólo se pueden comparar HMAC entre sí.
--
-- La intersección con `AND` de todos los trigramas de la consulta es lo que
-- hace el filtrado: cada trigrama solo es poco selectivo, pero exigir todos
-- deja fuera a la gran mayoría. El ranking fino se hace después en Rust
-- (Jaro-Winkler), donde ya se ha descifrado un conjunto pequeño de candidatos.
--
-- `array<string>` y no `set<string>`: el orden no importa para el `CONTAINS` y
-- el array serializa más compacto en las filas.
--
-- `DEFINE FIELD TYPE option<...>` porque las filas ya cifradas no tienen el
-- campo hasta que pase el backfill; un `array` estricto las haría fallar al
-- escribirse y, al no tener valor por defecto, SurrealDB rechazaría la lectura.

DEFINE FIELD IF NOT EXISTS bi_tng ON TABLE patients TYPE option<array<string>>;

-- Índice compuesto por tenant: la búsqueda siempre está acotada por tenant
-- (aislamiento PHI, SPEC-052), así que el tenant va delante.
DEFINE INDEX IF NOT EXISTS idx_patients_tenant_tng ON patients COLUMNS tenant_id, bi_tng;