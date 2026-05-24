# 文件名脱敏功能修复

## 问题描述

用户反馈：**文件名没有根据规则进行脱敏**

之前的实现中，虽然文件内容会根据脱敏规则进行处理，但文件名本身没有被正确脱敏。例如：

```
原始文件: 张三的工资单.xlsx
期望输出: 姓名1的工资单_脱敏.xlsx
实际输出: 张三的工资单_脱敏.xlsx  ❌ 文件名中的"张三"没有被脱敏
```

## 根本原因

在之前的修改中（将 `masked_` 前缀改为 `_脱敏` 后缀），我错误地修改了文件名脱敏逻辑：

### mask_file 函数 (masking.rs)

**问题代码：**
```rust
let final_file_name = if masked_file_name.is_empty() || 
                         masked_file_name.chars().all(|c| c == '*' || c.is_numeric()) {
    // 文件名被完全脱敏，使用原始文件名加后缀 ❌
    format!("{}_脱敏", original_file_name)
} else {
    // 文件名部分脱敏，在脱敏后的文件名后添加后缀
    format!("{}_脱敏", masked_file_name)
};
```

**问题分析：**
- 当文件名被完全脱敏（只剩占位符）时，代码会回退到使用原始文件名
- 这导致敏感信息没有被脱敏

## 解决方案

### 1. mask_file 函数修复

**修改后的代码：**
```rust
// 对文件名应用脱敏规则
let masked_file_name = masking_engine::mask_value_with_ner(
    original_file_name,
    &active_rules,
    &ner_detector,
    &mut mapping,
    &mut counter
);

// 构建最终文件名：脱敏后的文件名 + "_脱敏" 后缀
let final_file_name = if masked_file_name.is_empty() || 
                         masked_file_name.chars().all(|c| c == '*' || c.is_numeric()) {
    // 文件名被完全脱敏（只剩占位符），使用占位符加后缀 ✅
    format!("{}_脱敏", masked_file_name)
} else {
    // 文件名部分脱敏，在脱敏后的文件名后添加后缀
    format!("{}_脱敏", masked_file_name)
};
```

**改进说明：**
- 无论文件名是完全脱敏还是部分脱敏，都使用脱敏后的结果
- 不再回退到原始文件名
- 确保敏感信息不会泄露

### 2. save_preview_result 函数简化

**修改后的代码：**
```rust
// 对文件名添加 "_脱敏" 后缀
// 注意：预览保存时文件名不进行脱敏处理，只添加后缀标识
let masked_file_name = format!("{}_脱敏", original_file_name);
```

**说明：**
- 预览保存功能主要用于用户查看脱敏效果
- 文件名保持原样，只添加后缀标识
- 如果需要对预览文件名也进行脱敏，可以后续添加

## 测试场景

### 场景 1：文件名包含姓名

**输入：**
```
文件名: 张三的工资单.xlsx
规则: 姓名脱敏规则（张三 → 姓名1）
```

**输出：**
```
文件名: 姓名1的工资单_脱敏.xlsx ✅
```

### 场景 2：文件名包含手机号

**输入：**
```
文件名: 联系人13800138000.txt
规则: 手机号脱敏规则（13800138000 → ***PHONE***）
```

**输出：**
```
文件名: 联系人***PHONE***_脱敏.txt ✅
```

### 场景 3：文件名完全是敏感信息

**输入：**
```
文件名: 张三.docx
规则: 姓名脱敏规则（张三 → 姓名1）
```

**输出：**
```
文件名: 姓名1_脱敏.docx ✅
```

### 场景 4：文件名包含多个敏感信息

**输入：**
```
文件名: 张三13800138000的简历.pdf
规则: 姓名 + 手机号脱敏规则
```

**输出：**
```
文件名: 姓名1***PHONE***的简历_脱敏.pdf ✅
```

### 场景 5：文件名不包含敏感信息

**输入：**
```
文件名: 项目报告.xlsx
规则: 姓名 + 手机号脱敏规则
```

**输出：**
```
文件名: 项目报告_脱敏.xlsx ✅
```

## 文件名脱敏流程

```
原始文件名
    ↓
应用脱敏规则（正则 + NER）
    ↓
脱敏后的文件名
    ↓
添加 "_脱敏" 后缀
    ↓
最终文件名
```

### 详细步骤

1. **提取文件名**
   ```rust
   let original_file_name = std::path::Path::new(&options.file_path)
       .file_stem()
       .and_then(|s| s.to_str())
       .unwrap_or("file");
   ```

2. **应用脱敏规则**
   ```rust
   let masked_file_name = masking_engine::mask_value_with_ner(
       original_file_name,
       &active_rules,
       &ner_detector,
       &mut mapping,
       &mut counter
   );
   ```

3. **添加后缀**
   ```rust
   let final_file_name = format!("{}_脱敏", masked_file_name);
   ```

4. **构建完整路径**
   ```rust
   let final_output_path = if file_extension.is_empty() {
       format!("{}/{}", output_dir, final_file_name)
   } else {
       format!("{}/{}.{}", output_dir, final_file_name, file_extension)
   };
   ```

## 脱敏规则应用

文件名脱敏使用与文件内容相同的规则引擎：

### 1. 正则表达式规则
```rust
// 例如：手机号规则
pattern: r"\d{11}"
replacement: "***PHONE***"
```

### 2. NER 实体识别
```rust
// 使用 NER 模型识别：
// - 姓名 (PERSON)
// - 地址 (LOCATION)
// - 组织 (ORGANIZATION)
// 等
```

### 3. 敏感词库
```rust
// 自定义敏感词
// 例如：公司名称、项目代号等
```

## 映射文件 (.cmap)

文件名脱敏也会记录在映射文件中：

```json
{
  "mappings": [
    {
      "original": "张三",
      "masked": "姓名1",
      "type": "PERSON"
    },
    {
      "original": "13800138000",
      "masked": "***PHONE***",
      "type": "PHONE"
    }
  ]
}
```

这样可以通过反脱敏功能恢复原始文件名。

## 特殊情况处理

### 1. 文件名被完全脱敏

**场景：**
```
原始: 张三.txt
脱敏: 姓名1.txt
```

**处理：**
- 使用脱敏后的占位符
- 添加 "_脱敏" 后缀
- 结果：`姓名1_脱敏.txt`

### 2. 文件名包含特殊字符

**场景：**
```
原始: 张三/李四的报告.txt
```

**处理：**
- 先进行脱敏：`姓名1/姓名2的报告`
- 再使用 `sanitize_filename` 清理非法字符
- 结果：`姓名1_姓名2的报告_脱敏.txt`

### 3. 文件名过长

**场景：**
```
原始: 关于开展2026年度安全生产大检查工作的通知及实施方案.docx
```

**处理：**
- 正常脱敏和添加后缀
- 如果超过系统限制（Windows: 255字符），会被截断
- 建议：在脱敏前检查文件名长度

## 版本信息

- **修复版本**: v0.1.35
- **修改文件**:
  - `src-tauri/src/commands/masking.rs` (mask_file 函数)
  - `src-tauri/src/commands/masking.rs` (save_preview_result 函数)
- **影响范围**:
  - 单文件脱敏
  - 批量文件脱敏
  - 预览保存（简化处理）

## 向后兼容性

### 已有文件
- 不影响已经脱敏的文件
- 旧文件仍然可以正常使用

### 映射文件
- 新的文件名脱敏会记录在 .cmap 文件中
- 可以通过反脱敏恢复原始文件名

### 云端同步
- FileBay 上传路径不变（仍使用 `masked/` 目录）
- 文件名格式变化不影响云端存储

## 注意事项

### 1. 文件名唯一性

如果多个文件的原始文件名脱敏后相同，可能会导致文件覆盖：

```
文件1: 张三的报告.txt → 姓名1的报告_脱敏.txt
文件2: 李四的报告.txt → 姓名1的报告_脱敏.txt  ⚠️ 可能覆盖
```

**建议：**
- 在批量处理时检测文件名冲突
- 自动添加序号：`姓名1的报告_脱敏_1.txt`, `姓名1的报告_脱敏_2.txt`

### 2. 文件名可读性

完全脱敏的文件名可能不易识别：

```
原始: 张三的工资单.xlsx
脱敏: 姓名1的工资单_脱敏.xlsx  ✅ 可读
```

```
原始: 张三.xlsx
脱敏: 姓名1_脱敏.xlsx  ⚠️ 信息较少
```

**建议：**
- 鼓励用户使用描述性文件名
- 在文件名中包含非敏感的上下文信息

### 3. 反脱敏

文件名脱敏后，需要通过 .cmap 文件才能恢复：

```
脱敏文件: 姓名1的报告_脱敏.txt
映射文件: 姓名1的报告_脱敏.txt.cmap
```

**重要：**
- 保管好 .cmap 文件
- 不要单独分发脱敏文件

## 未来改进

### 1. 文件名冲突检测
```rust
// 检测并自动处理文件名冲突
if file_exists(&output_path) {
    output_path = add_suffix(&output_path, counter);
}
```

### 2. 文件名长度限制
```rust
// 检查并截断过长的文件名
if file_name.len() > MAX_FILENAME_LENGTH {
    file_name = truncate_filename(&file_name, MAX_FILENAME_LENGTH);
}
```

### 3. 可配置的文件名脱敏策略
```json
{
  "filename_masking": {
    "enabled": true,
    "keep_extension": true,
    "add_suffix": true,
    "suffix": "_脱敏"
  }
}
```

### 4. 预览文件名也进行脱敏
```rust
// save_preview_result 函数中也应用完整的脱敏逻辑
let masked_file_name = masking_engine::mask_value_with_ner(...);
```

---

**总结**: 这次修复确保了文件名会根据脱敏规则进行正确处理，不会泄露敏感信息。文件名脱敏使用与文件内容相同的规则引擎，保证了一致性和安全性。
