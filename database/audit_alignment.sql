-- 统计各置信度区间的歌曲数量
SELECT 
    width_bucket(lyric_confidence, 0, 1, 10) as confidence_bucket,
    COUNT(*) as track_count,
    AVG(lyric_confidence) as avg_conf
FROM tracks
WHERE lyric_alignment_strategy = 'forced_alignment_v4'
GROUP BY 1 ORDER BY 1;

-- 找出置信度低于 0.4 的“危险”歌曲
SELECT id, title, artist_name, lyric_confidence 
FROM tracks 
WHERE lyric_alignment_strategy = 'forced_alignment_v4' AND lyric_confidence < 0.4
LIMIT 20;
