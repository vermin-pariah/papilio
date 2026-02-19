# UI/UX 适配规范 (Mobile UI Standards)

## 1. 响应式布局原则
为了杜绝 Flutter 在不同 Android 机型上的“黄色警告方块” (Layout Overflow)，本项目强制执行以下规范：

### 动态尺寸计算
- **禁用硬编码高度**: 严禁在核心视图中使用 `height: 500` 等固定值。
- **使用 LayoutBuilder**: 在 `PlayerView` 等复杂界面，必须使用 `LayoutBuilder` 获取父容器高度，并根据高度动态调整元素比例。
- **Vinyl 比例规则**:
    - 高度 < 600px (窄屏): 唱片直径为 `screenWidth * 0.55`。
    - 高度 >= 600px (标准): 唱片直径为 `screenWidth * 0.70`。

## 2. 文本溢出防护
- 所有的 `ListTile` 标题和副标题必须包含 `maxLines: 1` 和 `overflow: TextOverflow.ellipsis`。
- 长列表操作栏 (Trailing Row) 必须包裹在 `BoxConstraints(maxWidth: 100)` 中，防止被长标题挤出界。

## 3. 滚动加固
- 复杂表单或详情页必须使用 `SingleChildScrollView` + `IntrinsicHeight` 组合，确保在系统字体放大或键盘弹出时不会触发溢出。

## 4. 歌词对齐体验
- 歌词滚动采用 `scrollable_positioned_list` 库。
- 焦点行居中比例固定为 `0.35` (QQ 音乐同款黄金视线位)。
- 拖动交互: 用户手动滑动时触发 5 秒锁定，随后自动恢复同步。
