# 系统架构白皮书 (System Architecture)

## 1. 全链路拓扑图 (Topology)

```mermaid
graph TD
    subgraph "External World"
        User[User (Mobile App)]
        MB[MusicBrainz API]
        WD[Wikidata API]
    end

    subgraph "Docker Host (Linux)"
        subgraph "Container: papilio-server"
            API[Axum API Layer]
            Scanner[Metadata Scanner]
            Stream[FFmpeg Streamer]
            Auth[Auth Guard]
        end

        subgraph "Container: db"
            PG[(PostgreSQL 15)]
        end

        subgraph "Container: redis"
            Redis[(Valkey/Redis)]
        end

        FS[Host File System (/music)]
    end

    User -->|HTTPS/JSON| API
    User -->|Stream (206)| Stream
    
    API --> Auth
    Auth --> Redis
    
    Scanner -->|Read Tags| FS
    Scanner -->|Query Metadata| MB
    Scanner -->|Query Info| WD
    Scanner -->|Write/Index| PG
    
    Stream -->|Read Audio| FS
    
    API -->|CRUD| PG
```

## 2. 核心组件交互

### 2.1 数据流转
- **元数据同步**: `Scanner` 定期扫描宿主机挂载的 `/music` 卷。读取 ID3 标签后，通过异步任务队列请求 MusicBrainz，获取的高清封面回写至 `/app/data/covers` 卷。
- **音频流**: 移动端发起 `GET /stream/{id}`。后端根据 `Range` 头直接透传文件字节流，或调用 FFmpeg 进程进行实时转码（如无损转 320k MP3 以适应弱网）。

### 2.2 存储设计
- **PostgreSQL**: 存储结构化关系数据（用户、歌单、元数据索引）。
- **Valkey (Redis)**: 存储短期会话 (Session)、API 速率限制计数器及扫描进度缓存。
- **文件系统**:
    - **只读**: `/music` (用户音源库)。
    - **读写**: `/app/data` (封面缓存、头像上传、日志)。

## 3. 安全架构
- **零信任上传**: 所有文件上传接口均经过 Magic Number 指纹校验。
- **网络隔离**: 容器间通过 Docker Network 通信，仅 API 端口 (3000) 对外暴露。
- **并发熔断**: 扫描器实现了基于内存锁的熔断机制，防止重入攻击导致的数据损坏。
