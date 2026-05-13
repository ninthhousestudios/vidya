-- Add slug column to sources for dedup during bulk loading
ALTER TABLE sources ADD COLUMN slug text;
UPDATE sources SET slug = id::text WHERE slug IS NULL;
ALTER TABLE sources ALTER COLUMN slug SET NOT NULL;
CREATE UNIQUE INDEX idx_sources_slug ON sources(slug);
