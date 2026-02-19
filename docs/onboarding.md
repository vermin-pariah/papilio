# Papilio 开发者上手指南 (Onboarding)

> **💡 提示**: 如果您只是想快速启动并使用 Papilio 生产环境，请直接参考 [**Docker 部署指南**](./docker_deployment.md)。

欢迎加入 Papilio 项目开发！这是一份旨在帮助你快速搭建环境并理解系统设计的指南。

## 🚀 环境准备

### 1. 基础工具
- **Rust**: 1.75+ (建议使用 stable)
- **Flutter**: 3.16+
- **Docker & Docker Compose**: 用于启动 Postgres 和 Valkey (Redis)
- **FFmpeg**: 系统路径中必须包含 ffmpeg，用于后端实时转码流

### 2. 启动基础设施
```bash
docker-compose up -d
```

### 3. 后端配置 (`papilio-server/.env`)
```env
DATABASE_URL=postgres://user:pass@localhost:5432/papilio
REDIS_URL=redis://127.0.0.1/
JWT_SECRET=your_jwt_secret_here
MUSIC_DIR=/your/music/library
```

## 🏗️ 构建与运行

### 后端 (Server)
```bash
cd papilio-server
cargo run
```
*提示：初次启动会自动运行数据库迁移并创建一个初始管理员账号 `chi` / `chi`。*

### 安卓端 (Mobile)
```bash
cd papilio_mobile
flutter run
```

## 🛡️ 安全与编码规范

1.  **上传安全**: 必须通过 `infer` 库校验文件头，严禁信任 MIME 报头。
2.  **路径处理**: 严禁直接拼接用户输入的字符串到文件路径，必须使用 `sanitize_filename` 进行净化。
3.  **并发锁**: 触发长耗时扫描任务时，必须通过 `Scanner::is_scanning()` 进行预检。
4.  **TDD**: 核心业务逻辑（尤其是 `papilio-core` 模块）必须配套对应的 `#[test]`。

## 📚 延伸阅读
- [ADR 0001: 附件上传与路径安全加固](./adr/0001-security-hardening.md)
- [ADR 0002: 扫描器并发冲突控制](./adr/0002-scanner-concurrency-control.md)
- [业务领域模型](./domain_model.md)
