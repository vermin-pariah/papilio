# 生产运维手册 (Operations Manual)

## 1. 服务管理

### 启动与重启
```bash
# 启动服务（后台模式）
docker compose up -d

# 重建镜像并重启（用于应用更新）
docker compose up -d --build server

# 查看实时日志
docker compose logs -f server
```

### 健康检查
系统内置了自愈机制。如果后端容器因 OOM 或 Panic 崩溃，Docker 会根据 `restart: unless-stopped` 策略自动重启。
- **启动自愈**: 每次服务启动时，`main.rs` 会自动重置数据库中挂起的 `is_scanning` 和 `is_syncing` 标志，无需人工干预。

## 2. 数据备份与恢复

### 数据库备份
建议每日执行一次全量备份：
```bash
# 导出 SQL 转储文件
docker compose exec db pg_dump -U postgres papilio > backup_$(date +%Y%m%d).sql
```

### 灾难恢复
如果数据库损坏，可使用以下命令恢复：
```bash
# 1. 停止应用
docker compose stop server

# 2. 恢复数据
cat backup_YYYYMMDD.sql | docker compose exec -T db psql -U postgres papilio

# 3. 重启应用
docker compose start server
```

## 3. 故障排查 (Troubleshooting)

### 扫描卡住
虽然已有自愈机制，但如果需要手动干预：
1. 进入数据库容器：`docker compose exec db psql -U postgres -d papilio`
2. 强制重置状态：
   ```sql
   UPDATE scan_status SET is_scanning = FALSE WHERE id = 1;
   UPDATE artist_sync_status SET is_syncing = FALSE WHERE id = 1;
   ```

### 资源耗尽
如果发现 FFmpeg 僵尸进程过多：
```bash
# 强制清理容器内所有 ffmpeg 进程
docker compose exec server pkill -9 ffmpeg
```

## 4. 环境变量参考
生产环境 `.env` 关键配置：
- `RUST_LOG`: 建议设为 `info`，调试时设为 `debug`。
- `SCAN_CONCURRENCY`: 扫描并发数，默认 8。磁盘 IO 较弱时建议降为 4。
