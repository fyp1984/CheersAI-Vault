# 批量 AI 处理实现完成

## 实现内容

### 1. 添加批量检测方法 (`ner.rs`)

**新增函数**: `detect_entities_batch(&self, texts: &[String]) -> Vec<Vec<EntityMatch>>`

**功能**:
- 接收多个文本
- 过滤掉不需要检测的文本（短文本、纯数字、标签等）
- 将所有有效文本合并成一个大文本
- 一次性调用 AI 检测
- 将检测结果映射回各个文本

**关键代码**:
```rust
// 合并文本
const SEPARATOR: &str = "\n###TEXT_SEPARATOR###\n";
let combined_text = valid_texts.join(SEPARATOR);

// 一次性检测
let all_entities = self.detect_entities(&combined_text);

// 映射回各个文本
for (i, text) in valid_texts.iter().enumerate() {
    // 找到属于当前文本的实体
    // 调整实体位置
}
```

### 2. 修改 CSV 处理逻辑 (`masking.rs`)

**之前**:
```rust
for row in rows {
    for cell in row {
        let masked = mask_value_with_ner(cell, ...);  // 每个单元格调用一次
    }
}
```

**现在**:
```rust
// 收集所有单元格
let all_cells: Vec<String> = rows.iter()
    .flat_map(|row| row.iter())
    .cloned()
    .collect();

// 批量检测
let batch_entities = ner_detector.detect_entities_batch(&all_cells);

// 应用脱敏
for (cell, entities) in all_cells.iter().zip(batch_entities.iter()) {
    let masked = apply_entities_to_text(cell, entities, ...);
}
```

### 3. 添加实体应用函数 (`masking_engine.rs`)

**新增函数**: `apply_entities_to_text(...)`

**功能**:
- 接收已检测到的实体列表
- 直接应用脱敏
- 生成映射关系

## 性能提升

### 理论提升

| 场景 | 之前 | 现在 | 提速 |
|------|------|------|------|
| 10个单元格 | 10秒 (10次AI调用) | 3秒 (1次AI调用) | 3.3x |
| 50个单元格 | 50秒 (50次AI调用) | 4秒 (1次AI调用) | 12.5x |
| 100个单元格 | 100秒 (100次AI调用) | 5秒 (1次AI调用) | 20x |
| 500个单元格 | 500秒 (500次AI调用) | 10秒 (1次AI调用) | 50x |

### 实际效果

需要测试验证，但预期：
- **小文件** (< 50个单元格): 提速 5-10倍
- **中文件** (50-200个单元格): 提速 10-20倍
- **大文件** (> 200个单元格): 提速 20-50倍

## 优化细节

### 1. 智能过滤
批量处理前会过滤掉：
- 空单元格
- 短文本 (< 5字符)
- 纯数字文本
- 数字占比 > 70% 的文本
- 包含表格符号的文本 (├ └ ─ 等)
- 包含标签关键词的短文本 ("阶段"、"照片"等)

### 2. 位置映射
- 记录每个文本在合并文本中的位置
- 检测到的实体位置需要调整为相对位置
- 确保实体准确映射回原文本

### 3. 错误处理
- 如果批量检测失败，不影响整个流程
- 空实体列表会回退到正则表达式脱敏

## 日志输出

### 批量处理日志
```
=== Batch entity detection for 100 texts ===
Filtered 45 valid texts from 100 total
Combined text length: 2345 chars
=== Starting multi-method entity detection (AI mode - parallel) ===
Regex detected 5 entities (took 2.1ms)
NER detected 3 entities (took 15.3ms)
Search detected 0 entities (took 1.8ms)
AI detected 8 entities (took 3.2s)
Final merged: 12 entities
Mapped entities back to 100 texts
Batch detection completed
```

### 性能对比
- **之前**: 每个单元格打印一次 "Starting multi-method entity detection"
- **现在**: 整个文件只打印一次

## 测试建议

### 测试 1: 小文件
1. 准备一个 10行 x 5列 的 Excel 文件
2. 开启 AI 检测
3. 处理文件
4. 预期时间: 3-5秒（之前需要 50秒）

### 测试 2: 中等文件
1. 准备一个 50行 x 10列 的 Excel 文件
2. 开启 AI 检测
3. 处理文件
4. 预期时间: 5-10秒（之前需要 500秒）

### 测试 3: 大文件
1. 准备一个 200行 x 10列 的 Excel 文件
2. 开启 AI 检测
3. 处理文件
4. 预期时间: 10-20秒（之前需要 2000秒）

### 测试 4: 验证准确性
1. 对比批量处理和逐个处理的结果
2. 确保检测到的实体一致
3. 确保脱敏结果正确

## 注意事项

### 1. AI 输入长度限制
- 如果合并后的文本超过 AI 模型限制（通常 4096 tokens）
- 可能需要分批处理
- 当前实现会一次性处理所有文本

### 2. 内存使用
- 批量处理会将所有单元格加载到内存
- 对于超大文件（> 10000个单元格），可能需要分批

### 3. Excel 处理
- 当前只实现了 CSV 的批量处理
- Excel 处理逻辑类似，需要同样的修改

## 后续优化

### 短期
1. 为 Excel 添加批量处理
2. 为 Word/PDF 添加批量处理（按段落）
3. 添加分批处理逻辑（处理超大文件）

### 中期
1. 添加进度反馈（显示批量处理进度）
2. 优化内存使用（流式处理）
3. 添加批量大小配置

### 长期
1. 使用本地 AI 模型（更快）
2. GPU 加速
3. 分布式处理

## 相关文件

- `cheersai-desktop/src-tauri/src/core/ner.rs` - 批量检测实现
- `cheersai-desktop/src-tauri/src/commands/masking.rs` - CSV 批量处理
- `cheersai-desktop/src-tauri/src/core/masking_engine.rs` - 实体应用函数

## 完成时间

2024-XX-XX

## 状态

✅ 实现完成
✅ 编译成功
⏳ 等待测试验证
