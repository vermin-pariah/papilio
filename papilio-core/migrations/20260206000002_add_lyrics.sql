-- 为 tracks 表增加歌词字段 (幂等化处理)
ALTER TABLE tracks ADD COLUMN IF NOT EXISTS lyrics TEXT;