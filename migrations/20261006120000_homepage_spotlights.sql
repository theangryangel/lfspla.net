-- Bound homepage activity scans without walking historical imports.
CREATE INDEX hotlap_spotlight_recent_idx ON hotlap (id DESC)
    WHERE source IN ('upload', 'demo') AND state = 'valid';
