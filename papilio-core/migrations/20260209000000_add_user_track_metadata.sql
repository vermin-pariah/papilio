-- 创建用户曲目元数据表，用于存储用户特定的设置（如歌词偏移）
CREATE TABLE user_track_metadata (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    track_id UUID NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    lyric_offset_ms INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, track_id)
);

-- 添加更新触发器
CREATE TRIGGER update_user_track_metadata_modtime 
BEFORE UPDATE ON user_track_metadata 
FOR EACH ROW EXECUTE PROCEDURE update_updated_at_column();
