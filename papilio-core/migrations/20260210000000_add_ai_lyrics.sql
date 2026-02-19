-- 增加 AI 逐字歌词支持
DO $$ BEGIN
    CREATE TYPE lyric_sync_status AS ENUM ('none', 'pending', 'processing', 'completed', 'failed');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

ALTER TABLE tracks ADD COLUMN IF NOT EXISTS lyrics_ai TEXT;
ALTER TABLE tracks ADD COLUMN IF NOT EXISTS sync_status lyric_sync_status DEFAULT 'none';

CREATE INDEX IF NOT EXISTS idx_tracks_lyric_sync ON tracks (sync_status) 
WHERE lyrics IS NOT NULL AND lyrics_ai IS NULL;
