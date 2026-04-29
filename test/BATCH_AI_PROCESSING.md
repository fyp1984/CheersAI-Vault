# 批量 AI 处理优化方案

## 当前问题

### 逐个单元格处理
```rust
for row in rows {
    for cell in row {
        // 每个单元格调用一次 AI
        let masked = mask_value_with_ner(cell, ...);  // AI调用: 1秒
    }
}

// 100个单元格 = 100次AI调用 = 100秒
```

## 优化方案：批量处理

### 方案 A: 全文批量（最快，推荐）

```rust
// 1. 收集所有单元格
let all_cells: Vec<String> = rows.iter()
    .flat_map(|row| row.iter())
    .cloned()
    .collect();

// 2. 合并成一个大文本
let combined_text = all_cells.join("\n###CELL###\n");

// 3. 一次AI调用检测所有实体
let entities = ner_detector.detect_entities(&combined_text);  // 1次AI调用: 3-5秒

// 4. 将实体映射回各个单元格
for (cell_idx, cell) in all_cells.iter().enumerate() {
    let cell_entities = filter_entities_for_cell(entities, cell_idx);
    apply_masking(cell, cell_entities);
}

// 100个单元格 = 1次AI调用 = 3-5秒
// 提速: 20-30倍！
```

### 方案 B: 分批处理（平衡）

```rust
const BATCH_SIZE: usize = 20;  // 每批20个单元格

for batch in all_cells.chunks(BATCH_SIZE) {
    let combined = batch.join("\n###CELL###\n");
    let entities = ner_detector.detect_entities(&combined);  // 1次AI调用
    apply_to_batch(batch, entities);
}

// 100个单元格 = 5次AI调用 = 15-25秒
// 提速: 4-6倍
```

## 实现细节

### 1. 文本合并策略

```rust
// 使用特殊分隔符
const CELL_SEPARATOR: &str = "\n###CELL_SEP###\n";

// 合并时添加索引
let combined = all_cells.iter()
    .enumerate()
    .map(|(idx, cell)| format!("[CELL_{}]{}", idx, cell))
    .collect::<Vec<_>>()
    .join(CELL_SEPARATOR);
```

### 2. 实体映射回单元格

```rust
fn map_entities_to_cells(
    entities: Vec<EntityMatch>,
    combined_text: &str,
    cells: &[String]
) -> HashMap<usize, Vec<EntityMatch>> {
    let mut cell_entities: HashMap<usize, Vec<EntityMatch>> = HashMap::new();
    
    // 计算每个单元格在合并文本中的位置
    let mut cell_positions = Vec::new();
    let mut current_pos = 0;
    
    for (idx, cell) in cells.iter().enumerate() {
        let start = current_pos;
        let end = start + cell.len();
        cell_positions.push((idx, start, end));
        current_pos = end + CELL_SEPARATOR.len();
    }
    
    // 将实体分配到对应的单元格
    for entity in entities {
        for (cell_idx, start, end) in &cell_positions {
            if entity.start >= *start && entity.end <= *end {
                // 调整实体位置为相对于单元格的位置
                let mut cell_entity = entity.clone();
                cell_entity.start -= start;
                cell_entity.end -= start;
                
                cell_entities.entry(*cell_idx)
                    .or_insert_with(Vec::new)
                    .push(cell_entity);
                break;
            }
        }
    }
    
    cell_entities
}
```

### 3. 应用脱敏

```rust
for (row_idx, row) in rows.iter().enumerate() {
    let mut masked_row = Vec::new();
    
    for (col_idx, cell) in row.iter().enumerate() {
        let cell_idx = row_idx * row.len() + col_idx;
        
        if let Some(entities) = cell_entities.get(&cell_idx) {
            // 应用实体脱敏
            let masked = apply_entities_to_text(cell, entities, &mut mapping, &mut counter);
            masked_row.push(masked);
        } else {
            // 没有检测到实体，保持原样
            masked_row.push(cell.clone());
        }
    }
    
    masked_rows.push(masked_row);
}
```

## 性能对比

| 场景 | 逐个处理 | 批量处理 | 提速 |
|------|---------|---------|------|
| 10个单元格 | 10秒 | 3秒 | 3.3x |
| 50个单元格 | 50秒 | 4秒 | 12.5x |
| 100个单元格 | 100秒 | 5秒 | 20x |
| 500个单元格 | 500秒 | 10秒 | 50x |

## 注意事项

### 1. AI 输入长度限制
- 大多数 AI 模型有输入长度限制（如 4096 tokens）
- 解决：使用分批处理，每批不超过限制

### 2. 实体位置准确性
- 合并文本后，实体位置需要重新计算
- 解决：记录每个单元格在合并文本中的位置

### 3. 空单元格处理
- 空单元格不应该参与 AI 调用
- 解决：过滤掉空单元格，只处理有内容的

### 4. 错误处理
- AI 调用失败不应影响整个文件
- 解决：失败时回退到逐个处理或跳过 AI

## 实现步骤

### 阶段 1: 基础批量处理（30分钟）
1. ✅ 修改 masking.rs，添加批量收集逻辑
2. ✅ 实现文本合并和分隔
3. ✅ 测试单次批量调用

### 阶段 2: 实体映射（30分钟）
1. ✅ 实现实体位置映射
2. ✅ 将实体分配回各个单元格
3. ✅ 测试映射准确性

### 阶段 3: 集成和优化（30分钟）
1. ✅ 集成到现有流程
2. ✅ 添加错误处理
3. ✅ 性能测试和调优

## 替代方案

### 方案 1: 关闭 AI 检测
- 最简单，立即生效
- 只使用正则表达式
- 速度提升 10-20倍

### 方案 2: 使用本地 AI 模型
- 更快的响应时间
- 无网络延迟
- 需要额外配置

### 方案 3: 智能采样
- 只对部分单元格调用 AI
- 其他单元格使用正则
- 平衡速度和准确性

## 建议

**立即可做**：
1. 实现批量 AI 处理（1-2小时）
2. 测试性能提升
3. 根据结果调整批次大小

**如果批量处理仍然慢**：
1. 考虑关闭 AI 检测
2. 或使用本地 AI 模型
3. 或实现智能采样

## 相关文件

- `cheersai-desktop/src-tauri/src/commands/masking.rs` - 脱敏处理
- `cheersai-desktop/src-tauri/src/core/ner.rs` - 实体检测
- `cheersai-desktop/src-tauri/src/core/masking_engine.rs` - 脱敏引擎
