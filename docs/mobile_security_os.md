# 安全与系统集成手册 (Mobile Security & OS)

## 1. 系统权限管理 (Android 13+)
本项目已适配最新的通知权限要求：
- **权限声明**: 在 `AndroidManifest.xml` 中定义 `POST_NOTIFICATIONS`。
- **请求时机**: 应用启动阶段，由 `permission_handler` 发起询问。
- **必要性**: 确保 `just_audio` 的前台播放控制器及下载通知能被系统正常展示。

## 2. 网络通信安全
- **明文通信白名单**: 彻底废弃全局 `cleartextTraffic` 标志。
- **配置路径**: `res/xml/network_security_config.xml`。
- **策略**: 
    - 允许 `localhost` 和 `127.0.0.1` 明文。
    - 允许 `192.168.0.0/16` 等常用局域网段明文（支持私有 NAS 服务器）。
    - 外部域名流量强制要求 **HTTPS**。

## 3. 会话自动治理
- **401 全局拦截**: 在 `ApiClient` 的 Dio 拦截器中，所有 `401 Unauthorized` 响应会自动触发 `authStateProvider` 的登出逻辑。
- **隐私保护**: 所有的密码输入框必须配套 `_obscurePassword` 状态及后缀 `suffixIcon` 可见性切换按钮。

## 4. 混淆加固
- 打包时已启用 `minifyEnabled true`。
- 混淆规则位于 `proguard-rules.pro`，已保留 `just_audio` 及 `sqlx` 相关的反射逻辑。
