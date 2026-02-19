-- 启用 UUID 扩展
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- 艺术家表
CREATE TABLE artists (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    bio TEXT,
    image_url TEXT,
    musicbrainz_artist_id UUID UNIQUE, -- MusicBrainz Artist ID
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_artists_name ON artists(name);

-- 专辑表
CREATE TABLE albums (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title TEXT NOT NULL,
    artist_id UUID NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    release_year INT,
    cover_path TEXT,
    musicbrainz_album_id UUID UNIQUE, -- MusicBrainz Release ID
    musicbrainz_release_group_id UUID, -- MusicBrainz Release Group ID
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_albums_title ON albums(title);
CREATE INDEX idx_albums_artist_id ON albums(artist_id);

-- 曲目表
CREATE TABLE tracks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title TEXT NOT NULL,
    album_id UUID REFERENCES albums(id) ON DELETE CASCADE,
    artist_id UUID REFERENCES artists(id) ON DELETE CASCADE,
    duration INT NOT NULL, -- 单位：秒
    track_number INT,
    disc_number INT DEFAULT 1,
    path TEXT NOT NULL UNIQUE,
    bitrate INT,
    format TEXT,
    size BIGINT,
    bpm INT,
    musicbrainz_track_id UUID UNIQUE, -- MusicBrainz Recording ID
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tracks_title ON tracks(title);
CREATE INDEX idx_tracks_album_id ON tracks(album_id);
CREATE INDEX idx_tracks_artist_id ON tracks(artist_id);

-- Telegram 来源记录表
CREATE TABLE telegram_sources (
    track_id UUID PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    chat_id BIGINT NOT NULL,
    message_id INT NOT NULL,
    file_reference BYTEA,
    downloaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 自动更新 updated_at 触发器函数
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 为各表添加触发器
CREATE TRIGGER update_artists_modtime BEFORE UPDATE ON artists FOR EACH ROW EXECUTE PROCEDURE update_updated_at_column();
CREATE TRIGGER update_albums_modtime BEFORE UPDATE ON albums FOR EACH ROW EXECUTE PROCEDURE update_updated_at_column();
CREATE TRIGGER update_tracks_modtime BEFORE UPDATE ON tracks FOR EACH ROW EXECUTE PROCEDURE update_updated_at_column();
