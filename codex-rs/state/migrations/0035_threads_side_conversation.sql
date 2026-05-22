ALTER TABLE threads ADD COLUMN side_parent_thread_id TEXT;
ALTER TABLE threads ADD COLUMN side_parent_turn_id TEXT;

CREATE INDEX idx_threads_side_parent
    ON threads(side_parent_thread_id, side_parent_turn_id);
