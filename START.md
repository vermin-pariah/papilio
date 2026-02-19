# Papilio 最终交付手册 (v1.0-stable)

欢迎使用 **Papilio** —— 一款致力于极致听感的、基于 Rust 与 Flutter 构建的高性能多用户私有云音乐中心。

---

## 🛠 核心技术特性
- **后端 (Rust/Axum)**: 高并发文件扫描、FFmpeg 实时转码流媒体、MusicBrainz 元数据自愈。
- **前端 (Flutter/Riverpod)**: 沉浸式 Material 3 设计、自适应动态主题、LRC 歌词同步、离线下载缓存。
- **协同 (Sync)**: 跨设备播放进度实时同步、管理员级系统管控。

---

## 🚀 快速启动

### 1. 后端环境部署
1. **安装依赖**: 确保系统已安装 `ffmpeg`, `libssl-dev`, `pkg-config` 及 `PostgreSQL`。
2. **初始化配置**: 在 `/mnt/data1/rust/papilio` 下编辑 `.env`:
   ```env
   DATABASE_URL=postgresql://postgres:root@localhost:5432/papilio
   JWT_SECRET=您的安全密钥
   MUSIC_DIR=/您的无损音乐目录
   COVER_DIR=/mnt/data1/rust/papilio/data/covers
   ```
3. **启动服务**:
   ```bash
   cd papilio-server
   cargo run --release
   ```

### 2. 移动端 App 运行
1. **同步环境**: 确保 Flutter 3.19.x 及 OpenJDK 21 已就绪。
2. **构建安装**:
   ```bash
   cd papilio_mobile
   flutter pub get
   flutter run --release
   ```
3. **联调配置**: 进入 App 后的“我的” -> “个人设置”，测试并保存您的服务器 IP 地址。

---

## 📖 进阶使用指南
- **同步曲库**: 具有 Admin 权限的用户可在设置页触发“同步曲库”，系统将自动抓取高清封面并补全年份信息。
- **省流量模式**: 在户外使用时，建议在设置中开启该模式，后端将自动执行 128kbps 实时转码。
- **离线模式**: 在专辑页点击“下载”，曲目将永久驻留在手机磁盘，支持断网秒开。

---
*Papilio - 让每一粒音符，在私有云端自由蝶变。*