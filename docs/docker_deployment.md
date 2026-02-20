# Docker 生产部署指南 (推荐)

Papilio 官方强烈推荐使用 **Docker** 进行生产环境部署。这种方式不仅能确保后端环境（如 FFmpeg 依赖）的完全一致，还提供了开箱即用的“启动自愈”能力。

## 🚀 快速开始

### 1. 准备配置文件
在您的服务器创建一个目录（如 `papilio`），并在其中创建 `docker-compose.yml`：

```yaml
services:
  server:
    image: andrialpcoulter/papilio-server:v0.1.0
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgresql://postgres:root@db:5432/papilio
      - REDIS_URL=redis://redis:6379/
      - JWT_SECRET=您的随机密钥
      - MUSIC_DIR=/music
    volumes:
      - /您的/物理曲库/路径:/music
      - ./data/covers:/app/data/covers
      - ./data/avatars:/app/data/avatars
    depends_on:
      db:
        condition: service_healthy
      redis:
        condition: service_started

  db:
    image: postgres:15-alpine
    environment:
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=root
      - POSTGRES_DB=papilio
    volumes:
      - ./data/postgres:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: valkey/valkey:7.2-alpine
    volumes:
      - ./data/redis:/data
```

### 2. 启动服务
```bash
docker compose up -d
```

> **🎉 部署成功！**
> 
> 服务启动后，您可以使用以下初始账号登录 App：
> - **用户名**: `chi`
> - **初始密码**: `chi`
> 
> 请在首次登录后，务必在“个人设置”中修改密码以确保系统安全。

## 🛠️ 核心配置说明

| 环境变量 | 说明 |
| :--- | :--- |
| `JWT_SECRET` | 必须修改。用于生成用户登录令牌，建议使用长随机字符串。 |
| `MUSIC_DIR` | 容器内的曲库路径，默认为 `/music`。请确保挂载了宿主机的物理目录。 |
| `SCAN_CONCURRENCY` | (可选) 扫描并发数。如果服务器配置较低，可设为 `4`。 |

## 📦 官方镜像
- **Docker Hub**: `andrialpcoulter/papilio-server:v0.1.0`
- **基础镜像**: Ubuntu 24.04 (包含全量音频解码与转码组件)

## 🛡️ 安全提示
1. **网络隔离**: 默认配置下，只有 3000 端口对外暴露。数据库和 Redis 仅在内部网络可见。
2. **持久化**: 务必保留 `./data` 目录的备份，这里存储了所有的数据库索引和高清封面。
