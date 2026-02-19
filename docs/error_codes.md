# 全局错误码规范 (Global Error Codes)

Papilio 采用 **HTTP 状态码 + 结构化 JSON** 的错误表达体系。

## 1. 响应格式
所有非 2xx 的响应均返回以下格式：
```json
{
  "error": "详细的错误信息描述"
}
```

## 2. 核心状态码映射

| 状态码 | 业务语意 | 典型场景 |
| :--- | :--- | :--- |
| **400** | Bad Request | 参数验证失败、播放列表名超长、文件过大 |
| **401** | Unauthorized | 未提供 Token、Token 已过期、Valkey Session 失效 |
| **403** | Forbidden | 普通用户尝试访问管理员接口、尝试修改他人播放列表 |
| **404** | Not Found | 歌曲/专辑不存在、物理文件在磁盘上缺失 |
| **422** | Unprocessable | 元数据服务故障（MusicBrainz 速率限制等） |
| **500** | Internal Error | 数据库连接断开、FFmpeg 进程崩溃、IO 异常 |

## 3. 特殊逻辑处理

### 认证拦截 (Auth Guard)
当移动端收到 **401** 状态码时，必须立即清除本地持久化的 `auth_token` 并强制跳转至登录页。

### 扫描锁拦截
当 `Scanner::is_scanning()` 返回 `true` 时，后端将返回 **400**，且提示词固定为 `"A scan is already in progress"`。前端应据此禁用扫描按钮。
