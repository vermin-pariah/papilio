-- 1. 清空數據以解決衝突
TRUNCATE TABLE tracks, albums, artists CASCADE;

-- 2. 為藝術家添加唯一約束 (基於名稱)
ALTER TABLE artists ADD CONSTRAINT artists_name_key UNIQUE (name);

-- 3. 為專輯添加唯一約束 (基於名稱 + 藝術家 ID)
ALTER TABLE albums ADD CONSTRAINT albums_title_artist_id_key UNIQUE (title, artist_id);
