# Papilio - 私有化高保真音乐流媒体系统

![Version](https://img.shields.io/badge/version-v0.1.0-blue) ![Build](https://img.shields.io/badge/build-passing-brightgreen) ![Security](https://img.shields.io/badge/security-hardened-green) ![Platform](https://img.shields.io/badge/platform-Android%20%7C%20Docker-lightgrey)

Papilio 是一个专注于性能与稳定性的私有音乐流媒体系统。它由 Rust 编写的高性能后端与 Flutter 编写的响应式移动端组成，旨在提供快速的本地曲库索引与高保真的音频传输体验。

## 🌟 核心特性

- **高效索引**: 基于 Axum + SQLx 的异步架构，支持高效的 SQL 批量更新。
- **智能扫描**: 
    - **元数据同步**: 自动索引本地文件，并通过 MusicBrainz 补全封面与信息。
    - **并发保护**: 内置全局扫描锁，确保数据库操作的原子性。
- **安全性**:
    - **文件校验**: 强制执行文件头指纹校验，防御非法文件上传。
    - **路径净化**: 严格过滤文件名非法字符，杜绝路径穿越风险。
- **移动端体验**:
    - **系统适配**: 深度适配 Android 13+ 通知权限，UI 响应式适配各尺寸屏幕。
    - **无损播放**: 原生支持 HTTP Range 请求，实现流媒体音质透传。

## 📚 文档导航

- [**Docker 部署 (推荐)**](docs/docker_deployment.md): 生产环境一键启动指南。
- [**上手指南**](docs/onboarding.md): 环境搭建与本地开发指令。
- [**架构规格**](docs/system_architecture.md): 系统拓扑图与组件说明。
- [**领域模型**](docs/domain_model.md): 业务逻辑与状态机定义。
- [**运维手册**](docs/operations_manual.md): 备份、故障排查与自愈机制。

## 🛠️ 技术栈

- **后端**: Rust, Axum, SQLx, PostgreSQL, Valkey
- **移动端**: Flutter, Riverpod, Just_Audio
- **基础设施**: Docker & Docker Compose

---
Copyright © 2026 Papilio Team.
