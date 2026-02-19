CREATE TABLE IF NOT EXISTS artist_sync_status (
    id INTEGER PRIMARY KEY DEFAULT 1,
    is_syncing BOOLEAN NOT NULL DEFAULT FALSE,
    current_count INTEGER NOT NULL DEFAULT 0,
    total_count INTEGER NOT NULL DEFAULT 0,
    last_sync_at TIMESTAMP WITH TIME ZONE,
    last_error TEXT,
    CONSTRAINT single_row_artist_sync CHECK (id = 1)
);
INSERT INTO artist_sync_status (id, is_syncing) VALUES (1, FALSE) ON CONFLICT DO NOTHING;
