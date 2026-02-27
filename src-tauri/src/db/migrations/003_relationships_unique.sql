-- Add UNIQUE constraint on relationships (source, target, type) for upsert safety.
-- Also add description column for human-readable relationship context.

CREATE UNIQUE INDEX IF NOT EXISTS idx_relationships_unique
    ON relationships(source_project_id, target_project_id, relationship_type);
