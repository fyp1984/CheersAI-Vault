# 剩余 Emoji 位置清单

## ✅ 所有高优先级文件已完成更新（2026-05-07）

### 已完成更新的文件

1. ✅ **src/components/file/OcrDownloadDialog.tsx**
   - 替换了 3 个 ✅ emoji 为 CheckCircle 图标

2. ✅ **src/components/file/FindReplaceDialog.tsx**
   - 💡 未检测到标准 PII → Lightbulb 图标
   - ⚠️ 未找到匹配内容 → AlertTriangle 图标

3. ✅ **src/components/browser/NativeBrowser.tsx**
   - 🚀 → Rocket 图标（注入脚本按钮）
   - 💡 使用提示 → Lightbulb 图标

4. ✅ **src/pages/CheersAICloudNew.tsx**
   - 💡 提示 → Lightbulb 图标

5. ✅ **src/pages/SandboxManager.tsx**
   - ⚠️ → AlertTriangle 图标（未设置 PIN 警告）
   - ✅ → CheckCircle2 图标（沙箱已解锁）

6. ✅ **src/pages/SensitiveTerms.tsx**
   - 💡 → Lightbulb 图标（使用提示）

7. ✅ **src/components/file/RuleSelector.tsx**
   - 💡 → Lightbulb 图标（敏感词库配置提示）

8. ✅ **src/components/VaultConfigSelector.tsx**
   - 📋 → AlertTriangle 图标（解决步骤标题）

9. ✅ **src/components/browser/EmbeddedBrowser.tsx**
   - 🔄 → RefreshCw 图标（强制跳转按钮）

10. ✅ **src/components/settings/GiteaSettings.tsx**
    - 💡 → Lightbulb 图标
    - ⚠️ → AlertTriangle 图标
    - ✅ → CheckCircle 图标

11. ✅ **src/pages/FileProcess.tsx**
    - 🤖 → Bot 图标
    - ⚠️ → AlertTriangle 图标

12. ✅ **src/components/file/FileManager.tsx**
    - 📁 → FolderOpen 图标

13. ✅ **src/pages/EnhancedServices.tsx**
    - 💡 → Lightbulb 图标
    - 🤖 → Bot 图标

14. ✅ **src/components/layout/MainLayout.tsx**
    - 在线/离线状态 → Globe/WifiOff 图标

## 剩余低优先级文件

以下文件为测试/开发页面，可在后续迭代中更新：

### 测试文件
- `src/pages/InstallerTest.tsx`
- `src/pages/TestPage.tsx`

这些文件不影响生产环境的用户体验，可以根据需要在后续版本中更新。

## 完整图标映射表

| Emoji | Lucide Icon | 尺寸 | 使用场景 |
|-------|-------------|------|---------|
| 💡 | `Lightbulb` | w-4 h-4 | 提示信息 |
| ⚠️ | `AlertTriangle` | w-4 h-4 | 警告信息 |
| ✅ | `CheckCircle` / `CheckCircle2` | w-4 h-4 | 成功状态 |
| ❌ | `XCircle` | w-4 h-4 | 错误状态 |
| 🚀 | `Rocket` | w-4 h-4 | 启动/执行 |
| 🔄 | `RefreshCw` | w-4 h-4 | 刷新/重试 |
| 📁 | `FolderOpen` | w-4 h-4 / w-12 h-12 | 文件夹 |
| 🤖 | `Bot` | w-4 h-4 / w-5 h-5 | AI功能 |
| 🌐 | `Globe` | w-4 h-4 | 在线状态 |
| 📡 | `WifiOff` | w-4 h-4 | 离线状态 |
| ℹ | `Info` | w-4 h-4 | 信息提示 |
| 📋 | `AlertTriangle` | w-4 h-4 | 步骤说明 |

## 更新进度

### 高优先级（用户界面）- 100% 完成
- [x] OcrDownloadDialog.tsx
- [x] FindReplaceDialog.tsx
- [x] NativeBrowser.tsx
- [x] CheersAICloudNew.tsx
- [x] SandboxManager.tsx
- [x] SensitiveTerms.tsx
- [x] RuleSelector.tsx
- [x] VaultConfigSelector.tsx
- [x] EmbeddedBrowser.tsx
- [x] GiteaSettings.tsx
- [x] FileProcess.tsx
- [x] FileManager.tsx
- [x] EnhancedServices.tsx
- [x] MainLayout.tsx

### 低优先级（测试文件）- 待定
- [ ] InstallerTest.tsx
- [ ] TestPage.tsx

## 更新总结

✅ **所有生产环境的用户界面文件已完成 emoji 到 icon 的替换**

- 总计更新文件：14 个
- 替换 emoji 数量：30+ 处
- 新增图标类型：12 种
- 符合 CheersAI UI 规范：✅

## 最后更新日期

2026-05-07
