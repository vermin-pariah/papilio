-- 增加逐字对齐歌词字段，存储 KTV 模式数据
ALTER TABLE tracks ADD COLUMN IF NOT EXISTS lyrics_word_aligned JSONB;

-- 结构示例:
-- [
--   { "text": "まるで", "start": 14.63, "end": 15.20, "words": [{ "char": "ま", "t": 14.63 }, { "char": "る", "t": 14.80 } ...] },
--   ...
-- ]
