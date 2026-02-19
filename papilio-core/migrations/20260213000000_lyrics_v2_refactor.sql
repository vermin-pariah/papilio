-- 歌词管理 V2：引入来源跟踪、物理偏移与同步状态机
-- 统一使用已有的 lyric_sync_status 枚举

-- 1. 创建来源枚举类型
DO $$ BEGIN
    CREATE TYPE lyrics_source_type AS ENUM ('file', 'embedded', 'online', 'none');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 2. 补全字段
ALTER TABLE tracks 
ADD COLUMN IF NOT EXISTS lyrics_source lyrics_source_type DEFAULT 'none',
ADD COLUMN IF NOT EXISTS lyric_offset_ms INTEGER DEFAULT 0;

-- 3. 确保 sync_status 默认值为 none
ALTER TABLE tracks ALTER COLUMN sync_status SET DEFAULT 'none'::lyric_sync_status;

-- 4. 数据刷洗：将已有歌词但未处理的记录设为 pending
UPDATE tracks SET sync_status = 'pending' WHERE lyrics IS NOT NULL AND sync_status = 'none';

-- 5. 创建/更新索引
CREATE INDEX IF NOT EXISTS idx_tracks_sync_status_v2 ON tracks (sync_status) WHERE sync_status = 'pending'::lyric_sync_status;
