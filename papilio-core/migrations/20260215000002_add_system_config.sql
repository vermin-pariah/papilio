-- 系统全局配置表
CREATE TABLE IF NOT EXISTS system_config (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 初始化默认配置：AI 默认关闭以节省资源
INSERT INTO system_config (key, value) 
VALUES ('ai_enabled', 'false'::jsonb)
ON CONFLICT (key) DO NOTHING;
