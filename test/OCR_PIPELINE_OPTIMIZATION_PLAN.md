# OCR 流水线优化方案

## 当前问题

### 现在的处理流程（串行）
```
PDF (100页)
  ↓
OCR 处理所有页面 (100秒)
  ↓
等待 OCR 完成
  ↓
开始脱敏处理 (50秒)
  ↓
总耗时: 150秒
```

### 问题
1. OCR 处理完所有页面才开始脱敏
2. 脱敏等待 OCR，CPU 空闲
3. 无法并行利用资源

## 优化方案：流水线处理

### 新的处理流程（流水线）
```
页面1: OCR (1秒) → 脱敏 (0.5秒) ┐
页面2: OCR (1秒) → 脱敏 (0.5秒) ├→ 并行执行
页面3: OCR (1秒) → 脱敏 (0.5秒) │
...                              │
页面100: OCR (1秒) → 脱敏 (0.5秒)┘

总耗时: max(100秒 OCR, 50秒脱敏) ≈ 100秒
提速: 150秒 → 100秒 (33% faster)
```

### 优势
1. **OCR 和脱敏并行**：OCR 处理页面N时，同时脱敏页面N-1
2. **更快的响应**：第一页处理完就能看到结果
3. **更好的资源利用**：CPU 和 I/O 同时工作

## 技术实现

### 方案 A: 简单流式处理（推荐）

#### 1. 修改 Python 脚本
```python
def extract_text_from_pdf_streaming(pdf_path):
    """逐页输出 OCR 结果"""
    doc = fitz.open(pdf_path)
    ocr = get_ocr_reader()
    
    for page_num in range(len(doc)):
        page = doc[page_num]
        # ... OCR 处理 ...
        
        # 立即输出当前页结果（JSON 格式）
        result = {
            "page": page_num + 1,
            "text": page_text,
            "total_pages": len(doc)
        }
        print(json.dumps(result, ensure_ascii=False))
        sys.stdout.flush()  # 强制刷新输出
```

#### 2. 修改 Rust 端
```rust
fn process_pdf_with_streaming_ocr(pdf_path: &str) -> Result<String> {
    use std::process::{Command, Stdio};
    use std::io::{BufRead, BufReader};
    use std::thread;
    use std::sync::mpsc;
    
    // 启动 OCR 进程
    let mut child = Command::new("python")
        .arg("pdf_ocr.py")
        .arg(pdf_path)
        .stdout(Stdio::piped())
        .spawn()?;
    
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    
    // 创建通道用于页面处理
    let (tx, rx) = mpsc::channel();
    
    // 线程1: 读取 OCR 结果
    thread::spawn(move || {
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Ok(page_data) = serde_json::from_str(&line) {
                    tx.send(page_data).unwrap();
                }
            }
        }
    });
    
    // 线程2: 处理脱敏
    let mut results = Vec::new();
    while let Ok(page_data) = rx.recv() {
        // 立即开始脱敏处理
        let masked_text = mask_text(&page_data.text);
        results.push(masked_text);
    }
    
    Ok(results.join("\n\n"))
}
```

### 方案 B: 多线程池处理（更复杂）

使用线程池同时处理多个页面的脱敏：

```rust
use rayon::prelude::*;

// 收集所有页面
let pages: Vec<PageData> = collect_pages_from_ocr();

// 并行脱敏
let masked_pages: Vec<String> = pages
    .par_iter()
    .map(|page| mask_text(&page.text))
    .collect();
```

## 实现步骤

### 阶段 1: 基础流式处理（1-2小时）
1. ✅ 修改 `pdf_ocr.py`，添加流式输出模式
2. ✅ 修改 `file_parser.rs`，实时读取 OCR 结果
3. ✅ 测试单页处理流程

### 阶段 2: 并行脱敏（1小时）
1. ✅ 为每个页面创建脱敏任务
2. ✅ 使用线程池并行处理
3. ✅ 合并结果并保持页面顺序

### 阶段 3: 进度反馈（30分钟）
1. ✅ 实时更新处理进度
2. ✅ 显示当前处理的页面
3. ✅ 估算剩余时间

## 预期效果

### 性能提升
| 场景 | 之前 | 之后 | 提升 |
|------|------|------|------|
| 10页 PDF | 15秒 | 10秒 | 33% |
| 50页 PDF | 75秒 | 50秒 | 33% |
| 100页 PDF | 150秒 | 100秒 | 33% |

### 用户体验
- ✅ 更快看到第一页结果
- ✅ 实时进度更新
- ✅ 可以提前取消

## 风险和注意事项

### 1. 内存使用
- 流式处理会占用更多内存（多个页面同时在内存中）
- 解决：限制并行处理的页面数量（如最多5页）

### 2. 错误处理
- 某一页 OCR 失败不应影响其他页
- 解决：每页独立错误处理，失败页面标记为"处理失败"

### 3. 顺序保证
- 并行处理可能导致页面顺序混乱
- 解决：使用页码标记，最后按顺序合并

## 替代方案

如果流水线实现复杂，可以考虑：

### 方案 1: 批量处理
- 每10页为一批
- 批内并行处理
- 批间串行

### 方案 2: 预处理缓存
- 第一次 OCR 后缓存结果
- 后续处理直接使用缓存
- 适合需要多次处理同一文件的场景

### 方案 3: 关闭 AI 检测
- 最简单的优化
- 只使用正则表达式
- 速度提升 10-20 倍

## 建议

**短期（立即可做）**：
1. 关闭 AI 检测，使用正则表达式
2. 添加更多文本过滤规则
3. 优化现有的并行检测

**中期（1-2天）**：
1. 实现 OCR 流水线处理
2. 添加进度反馈
3. 优化内存使用

**长期（1周+）**：
1. 使用本地 AI 模型替代在线 API
2. GPU 加速 OCR
3. 分布式处理（多机器）

## 相关文件

- `cheersai-desktop/src-tauri/scripts/pdf_ocr.py` - OCR 脚本
- `cheersai-desktop/src-tauri/src/core/file_parser.rs` - 文件解析
- `cheersai-desktop/src-tauri/src/commands/masking.rs` - 脱敏处理
