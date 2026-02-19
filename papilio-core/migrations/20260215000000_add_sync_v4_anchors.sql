-- 增加对齐锚点表，用于 AI Sync 4.0
CREATE TABLE track_alignment_anchors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    track_id UUID NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    lrc_index INTEGER NOT NULL,
    audio_start_secs FLOAT NOT NULL,
    audio_end_secs FLOAT NOT NULL,
    confidence FLOAT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_anchors_track_id ON track_alignment_anchors(track_id);

-- 扩展 tracks 表以支持 4.0 状态
ALTER TABLE tracks ADD COLUMN IF NOT EXISTS lyric_alignment_strategy TEXT;
ALTER TABLE tracks ADD COLUMN IF NOT EXISTS lyric_confidence FLOAT;
ALTER TABLE tracks ADD COLUMN IF NOT EXISTS lyric_sync_version INTEGER DEFAULT 3;
