# 文件拖拽功能问题

## 问题描述
在 Windows 环境下，Tauri v2 的文件拖拽功能无法正常工作。虽然鼠标光标在拖拽时会变化（说明系统识别了拖拽操作），但 Tauri 的拖拽事件完全没有被触发。

## 最新修复尝试 (2026-04-24)

### 使用正确的 Tauri v2 事件名称
根据 Tauri v2 官方文档，正确的文件拖放事件名称为：
- `tauri://drag-enter` - 文件进入窗口
- `tauri://drag-over` - 文件在窗口上方移动
- `tauri://drag-drop` - 文件被拖放
- `tauri://drag-leave` - 拖放离开窗口

之前使用的事件名（`tauri://drag-hover`, `tauri://drag-cancelled`）是错误的。

### 当前实现
```typescript
// 使用 getCurrentWindow().listen() API
const window = getCurrentWindow();

window.listen<string[]>('tauri://drag-enter', (event) => {
  setIsDragActive(true);
});

window.listen<string[]>('tauri://drag-drop', (event) => {
  setIsDragActive(false);
  if (event.payload && event.payload.length > 0) {
    onFilesDropped(event.payload);
  }
});

window.listen('tauri://drag-leave', () => {
  setIsDragActive(false);
});
```

### 重要说明
- Tauri 默认启用文件拖放功能（`dragDropEnabled: true`）
- 如果要使用 HTML5 原生 drag and drop，需要设置 `dragDropEnabled: false`
- 我们使用 Tauri 的文件拖放系统，所以保持默认配置

## 已尝试的解决方案

### 1. 配置文件修改
- ✅ 添加了窗口标签 `"label": "main"`
- ❌ 尝试添加 `dragDropEnabled` 和 `fileDropEnabled`（均不是有效配置项）

### 2. 事件监听方式
- ✅ 尝试了 `window.onDragDropEvent()` API（方法不存在）
- ✅ 尝试了错误的事件名 `tauri://file-drop`, `tauri://drag-hover`
- ✅ 现在使用正确的事件名 `tauri://drag-enter`, `tauri://drag-drop`, `tauri://drag-leave`

### 3. HTML5 拖拽 API
- ❌ HTML5 原生拖拽事件在 Tauri WebView 中不工作（被 Tauri 的文件拖放系统覆盖）

## 问题分析

### 症状
1. 鼠标光标在拖拽时会变化 ✓
2. 但没有任何 JavaScript 事件被触发 ✗
3. 没有任何 Rust 端日志输出 ✗
4. 点击选择文件功能正常工作 ✓

### 可能的原因
1. **Tauri v2 在 Windows 上的 bug**：GitHub issue #9448 报告了相同的问题
2. **Windows 权限问题**：可能需要特殊的权限配置
3. **WebView2 限制**：Windows 的 WebView2 可能阻止了文件拖拽

## 临时解决方案

### 方案 1：使用点击选择文件（当前可用）
用户可以点击选择框来打开文件选择对话框，这个功能是正常工作的。

### 方案 2：检查 Tauri GitHub Issues
相关问题：
- https://github.com/tauri-apps/tauri/issues/9448 - Tauri Drag and Drop Events Not Firing on Windows
- https://github.com/tauri-apps/tauri/discussions/9696 - drag/drop and 1.x / 2.x documentation

### 方案 3：等待 Tauri 修复
这可能是 Tauri v2 在 Windows 上的已知 bug，需要等待官方修复。

## 相关代码位置

- 前端组件：`src/components/file/DropZone.tsx`
- 后端配置：`src-tauri/src/lib.rs`
- 窗口配置：`src-tauri/tauri.conf.json`

## 参考资料

- [Tauri v2 Event API](https://docs.crabnebula.dev/taurify/api/namespaceevent/)
- [GitHub Issue #9448](https://github.com/tauri-apps/tauri/issues/9448)
- [GitHub Issue #14373](https://github.com/tauri-apps/tauri/issues/14373) - dragDropEnabled 说明

## 日期
- 初次记录：2026-04-23
- 最后更新：2026-04-24
