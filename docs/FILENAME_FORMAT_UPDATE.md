# 文件名格式优化

## 问题描述

之前脱敏后的文件使用 `masked_` 前缀命名，导致：
1. 文件名过长时显示不全
2. 前缀占用空间，原始文件名被截断
3. 不符合中文用户的阅读习惯

**示例问题：**
```
masked_关于开展安全生产大检查的通知.txt
       ↑ 前缀占用 7 个字符
```

在文件管理器中显示为：
```
masked_关于开展安...
```

## 解决方案

将文件命名格式从 **前缀** 改为 **后缀**：

### 修改前
```
masked_原始文件名.扩展名
```

### 修改后
```
原始文件名_脱敏.扩展名
```

## 优势

### 1. 文件名更易读
```
之前: masked_关于开展安全生产大检查的通知.txt
现在: 关于开展安全生产大检查的通知_脱敏.txt
```

### 2. 显示更完整
在文件管理器中：
```
之前: masked_关于开展安...
现在: 关于开展安全生产大检查的通知_脱敏.txt
```

### 3. 符合中文习惯
- 中文用户习惯从左到右阅读
- 重要信息（原始文件名）在前
- 状态标识（_脱敏）在后

### 4. 排序更合理
在文件列表中按名称排序时：
```
之前:
  masked_报告A.txt
  masked_报告B.txt
  masked_通知A.txt

现在:
  报告A_脱敏.txt
  报告B_脱敏.txt
  通知A_脱敏.txt
```

## 实现细节

### 后端修改

#### 1. batch.rs - 批量处理
```rust
// 修改前
let safe_file_name = sanitize_filename(&format!("masked_{}", file_name));

// 修改后
let safe_file_name = if let Some(dot_pos) = file_name.rfind('.') {
    let name_part = &file_name[..dot_pos];
    let ext_part = &file_name[dot_pos..];
    sanitize_filename(&format!("{}_脱敏{}", name_part, ext_part))
} else {
    sanitize_filename(&format!("{}_脱敏", file_name))
};
```

**逻辑说明：**
- 查找最后一个 `.` 的位置
- 将文件名分为名称部分和扩展名部分
- 在名称和扩展名之间插入 `_脱敏`

**示例：**
```
输入: "报告.txt"
输出: "报告_脱敏.txt"

输入: "数据.backup.csv"
输出: "数据.backup_脱敏.csv"

输入: "文件"
输出: "文件_脱敏"
```

#### 2. masking.rs - mask_file 函数
```rust
// 修改前
let final_file_name = if masked_file_name.is_empty() || 
                         masked_file_name.chars().all(|c| c == '*' || c.is_numeric()) {
    format!("masked_file_{}", counter)
} else {
    masked_file_name
};

// 修改后
let final_file_name = if masked_file_name.is_empty() || 
                         masked_file_name.chars().all(|c| c == '*' || c.is_numeric()) {
    format!("{}_脱敏", original_file_name)
} else {
    format!("{}_脱敏", masked_file_name)
};
```

**逻辑说明：**
- 如果文件名被完全脱敏（只剩占位符），使用原始文件名
- 如果文件名部分脱敏，使用脱敏后的文件名
- 统一添加 `_脱敏` 后缀

#### 3. masking.rs - save_preview_result 函数
```rust
// 修改前
if result.is_empty() || result.chars().all(|c| c == '[' || c == ']' || c.is_alphabetic()) {
    format!("masked_{}", original_file_name.chars().take(5).collect::<String>())
} else {
    result
}

// 修改后
if result.is_empty() || result.chars().all(|c| c == '[' || c == ']' || c.is_alphabetic()) {
    format!("{}_脱敏", original_file_name)
} else {
    format!("{}_脱敏", result)
}
```

### 前端修改

#### FileManager.tsx
```tsx
// 修改前
{file.name.includes('masked') && (
  <span className="...">脱敏</span>
)}
<span>{file.name.startsWith('masked_') ? file.name.substring(7) : file.name}</span>

// 修改后
{(file.name.includes('masked') || file.name.includes('_脱敏')) && (
  <span className="...">脱敏</span>
)}
<span>{file.name}</span>
```

**逻辑说明：**
- 检测 `masked` 或 `_脱敏` 来显示标签
- 直接显示完整文件名，不再去除前缀
- 使用 `truncate` 和 `max-w-xl` 处理过长文件名

## 兼容性

### 旧文件支持
- 前端仍然识别 `masked` 关键字
- 旧的 `masked_` 文件仍会显示"脱敏"标签
- 不影响已有文件的使用

### 新旧文件对比
```
旧格式: masked_报告.txt     [脱敏] masked_报告.txt
新格式: 报告_脱敏.txt       [脱敏] 报告_脱敏.txt
```

## 测试场景

### 1. 普通文件名
```
输入: "报告.txt"
输出: "报告_脱敏.txt"
```

### 2. 长文件名
```
输入: "关于开展安全生产大检查的通知.txt"
输出: "关于开展安全生产大检查的通知_脱敏.txt"
```

### 3. 多扩展名
```
输入: "数据.backup.csv"
输出: "数据.backup_脱敏.csv"
```

### 4. 无扩展名
```
输入: "README"
输出: "README_脱敏"
```

### 5. 文件名包含敏感词
```
输入: "张三的工资单.xlsx"
处理: 文件名脱敏 -> "姓名1的工资单"
输出: "姓名1的工资单_脱敏.xlsx"
```

### 6. 文件名完全是敏感词
```
输入: "张三.txt"
处理: 文件名脱敏 -> "姓名1"
输出: "姓名1_脱敏.txt"
```

## 用户体验改进

### 文件列表显示
```
之前:
┌────────────────────────────────┐
│ [脱敏] masked_关于开展安...     │
└────────────────────────────────┘

现在:
┌────────────────────────────────┐
│ [脱敏] 关于开展安全生产大检查的通知_脱敏.txt │
└────────────────────────────────┘
```

### 文件系统显示
在 Windows 资源管理器中：
```
之前:
  masked_关于开展安全生产大检查的通知.txt
  masked_深圳市网项目策划.md

现在:
  关于开展安全生产大检查的通知_脱敏.txt
  深圳市网项目策划_脱敏.md
```

### 搜索体验
用户搜索 "安全生产" 时：
```
之前: 需要记住文件名前有 "masked_" 前缀
现在: 直接搜索原始文件名即可
```

## 版本信息

- **修改版本**: v0.1.35
- **修改文件**:
  - `src-tauri/src/core/batch.rs`
  - `src-tauri/src/commands/masking.rs`
  - `src/components/file/FileManager.tsx`
- **向后兼容**: 是（仍识别旧的 `masked_` 格式）

## 注意事项

### 1. 文件名长度限制
- Windows: 最大 255 字符
- 添加 `_脱敏` 后缀会增加 3 个字符（UTF-8）
- 如果原文件名接近限制，可能需要截断

### 2. 特殊字符处理
- 使用 `sanitize_filename` 确保跨平台兼容
- 自动处理非法字符（如 `/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`）

### 3. 云端同步
- FileBay 上传路径仍使用 `masked/` 目录
- 文件名格式变化不影响云端存储结构

## 未来改进

### 1. 可配置后缀
允许用户自定义后缀：
```rust
// 配置文件
{
  "masked_suffix": "_脱敏",  // 或 "_masked", "_processed" 等
}
```

### 2. 时间戳选项
可选添加时间戳：
```
报告_脱敏_20260516.txt
```

### 3. 批次标识
批量处理时添加批次号：
```
报告_脱敏_batch001.txt
```

---

**总结**: 这次修改将文件命名从前缀改为后缀，显著提升了文件名的可读性和用户体验，特别是对于中文文件名。同时保持了向后兼容性，不影响已有文件的使用。
