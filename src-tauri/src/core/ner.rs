use regex::Regex;
use serde::{Serialize, Deserialize};
use std::collections::{HashSet, HashMap};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub text: String,
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
    pub source: String, // 检测来源：ai, ner, regex, search
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowEntities {
    pub row_index: usize,
    pub entities: Vec<EntityMatch>,
}

#[derive(Clone)]
pub struct NERDetector {
    patterns: Vec<(String, Regex)>,
    common_surnames: HashSet<String>,
    name_context_keywords: Vec<String>,
    use_ai_detection: bool, // 是否使用 AI 检测
}

impl NERDetector {
    pub fn new() -> Self {
        Self::new_with_ai_detection(false)
    }
    
    pub fn new_with_ai_detection(use_ai: bool) -> Self {
        let mut patterns = Vec::new();
        
        // 优先级从高到低排列，更具体的模式放在前面
        
        // 中国身份证号（18位）
        patterns.push((
            "身份证号".to_string(),
            Regex::new(r"\b[1-9]\d{5}(18|19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[\dXx]\b").unwrap()
        ));
        
        // 手机号（11位，1开头）
        patterns.push((
            "手机号".to_string(),
            Regex::new(r"\b1[3-9]\d{9}\b").unwrap()
        ));
        
        // 邮箱地址
        patterns.push((
            "邮箱".to_string(),
            Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap()
        ));
        
        // IPv4 地址
        patterns.push((
            "IP地址".to_string(),
            Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()
        ));
        
        // 银行卡号（16-19位数字）
        patterns.push((
            "银行卡号".to_string(),
            Regex::new(r"\b\d{16,19}\b").unwrap()
        ));
        
        // 护照号（中国护照：E/G/P开头+8位数字）
        patterns.push((
            "护照号".to_string(),
            Regex::new(r"\b[EGP]\d{8}\b").unwrap()
        ));
        
        // 日期格式（多种格式）
        patterns.push((
            "日期".to_string(),
            Regex::new(r"\d{4}[-/年]\d{1,2}[-/月]\d{1,2}[日]?").unwrap()
        ));
        
        // 中国省份和直辖市
        patterns.push((
            "地名".to_string(),
            Regex::new(r"(北京|上海|天津|重庆|河北|山西|辽宁|吉林|黑龙江|江苏|浙江|安徽|福建|江西|山东|河南|湖北|湖南|广东|海南|四川|贵州|云南|陕西|甘肃|青海|台湾|内蒙古|广西|西藏|宁夏|新疆|香港|澳门)(省|市|自治区|特别行政区)?").unwrap()
        ));
        
        // 金额（带货币符号）
        patterns.push((
            "金额".to_string(),
            Regex::new(r"[¥$€£]\s?\d+(?:,\d{3})*(?:\.\d{2})?").unwrap()
        ));
        
        // 地址（包含省市区县等关键词）
        patterns.push((
            "地址".to_string(),
            Regex::new(r"[\u4e00-\u9fa5]{2,}[省市区县镇村][\u4e00-\u9fa5\d]{2,}[街道路巷号楼室单元][\u4e00-\u9fa5\d]*").unwrap()
        ));
        
        // 初始化常见姓氏列表（中国百家姓前100位）
        let mut common_surnames = HashSet::new();
        let surnames = vec![
            "王", "李", "张", "刘", "陈", "杨", "黄", "赵", "周", "吴",
            "徐", "孙", "朱", "马", "胡", "郭", "林", "何", "高", "梁",
            "郑", "罗", "宋", "谢", "唐", "韩", "曹", "许", "邓", "萧",
            "冯", "曾", "程", "蔡", "彭", "潘", "袁", "于", "董", "余",
            "苏", "叶", "吕", "魏", "蒋", "田", "杜", "丁", "沈", "姜",
            "范", "江", "傅", "钟", "卢", "汪", "戴", "崔", "任", "陆",
            "廖", "姚", "方", "金", "邱", "夏", "谭", "韦", "贾", "邹",
            "石", "熊", "孟", "秦", "阎", "薛", "侯", "雷", "白", "龙",
            "段", "郝", "孔", "邵", "史", "毛", "常", "万", "顾", "赖",
            "武", "康", "贺", "严", "尹", "钱", "施", "牛", "洪", "龚"
        ];
        for surname in surnames {
            common_surnames.insert(surname.to_string());
        }
        
        // 上下文关键词（姓名前后可能出现的词）
        let name_context_keywords = vec![
            "联系人".to_string(), "负责人".to_string(), "项目负责人".to_string(),
            "姓名".to_string(), "经理".to_string(), "总监".to_string(),
            "主任".to_string(), "专员".to_string(), "工程师".to_string(),
            "联系".to_string(), "对接人".to_string(), "接口人".to_string(),
        ];
        
        Self { 
            patterns,
            common_surnames,
            name_context_keywords,
            use_ai_detection: use_ai,
        }
    }
    
    /// 主检测函数：使用四种方法检测实体，姓名取交集，其他取并集
    pub fn detect_entities(&self, text: &str) -> Vec<EntityMatch> {
        // 跳过太短的文本，避免不必要的 AI 调用
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.len() < 5 {
            println!("Skipping detection for short text ({} chars)", trimmed.len());
            return Vec::new();
        }
        
        // 跳过纯数字、金额、日期等格式化数字
        let is_numeric_like = trimmed.chars().all(|c| {
            c.is_numeric() || c.is_whitespace() || 
            c == '.' || c == ',' || c == '-' || c == '/' || 
            c == ':' || c == '%' || c == '$' || c == '¥' ||
            c == '(' || c == ')'
        });
        
        if is_numeric_like {
            println!("Skipping detection for numeric/formatted text: {}", trimmed);
            return Vec::new();
        }
        
        // 跳过看起来像金额的文本（主要是数字和少量符号）
        let digit_count = trimmed.chars().filter(|c| c.is_numeric()).count();
        let total_count = trimmed.chars().filter(|c| !c.is_whitespace()).count();
        if total_count > 0 && (digit_count as f32 / total_count as f32) > 0.7 {
            println!("Skipping detection for number-heavy text ({}% digits): {}", 
                (digit_count as f32 / total_count as f32 * 100.0) as i32, trimmed);
            return Vec::new();
        }
        
        // 跳过表头和标签（包含大量符号或格式字符）
        let symbol_chars = trimmed.chars().filter(|c| {
            *c == '├' || *c == '└' || *c == '─' || *c == '│' || 
            *c == '┌' || *c == '┐' || *c == '┘' || *c == '┴' ||
            *c == '：' || *c == '、' || *c == '《' || *c == '》' ||
            *c == '【' || *c == '】' || *c == '（' || *c == '）'
        }).count();
        
        if symbol_chars > 2 {
            println!("Skipping detection for label/header text (too many symbols): {}", trimmed);
            return Vec::new();
        }
        
        // 跳过看起来像标题或标签的短文本（少于15个字符且包含"阶段"、"步骤"等关键词）
        if trimmed.len() < 15 {
            let label_keywords = ["阶段", "步骤", "照片", "图片", "附件", "说明", "备注", "金额", "数量", "单价", "合计"];
            if label_keywords.iter().any(|kw| trimmed.contains(kw)) {
                println!("Skipping detection for label text: {}", trimmed);
                return Vec::new();
            }
        }
        
        if self.use_ai_detection {
            // AI 模式：使用四种方法并行处理
            println!("=== Starting multi-method entity detection (AI mode - parallel) ===");
            
            use std::sync::Arc;
            use std::thread;
            
            // 将 text 包装为 Arc 以便在多线程间共享
            let text_arc = Arc::new(text.to_string());
            let self_arc = Arc::new(self.clone());
            
            // 创建四个线程，每个处理一种检测方法
            let text1 = Arc::clone(&text_arc);
            let self1 = Arc::clone(&self_arc);
            let handle_regex = thread::spawn(move || {
                let start = std::time::Instant::now();
                let result = self1.detect_with_regex(&text1);
                println!("Regex detected {} entities (took {:?})", result.len(), start.elapsed());
                result
            });
            
            let text2 = Arc::clone(&text_arc);
            let self2 = Arc::clone(&self_arc);
            let handle_ner = thread::spawn(move || {
                let start = std::time::Instant::now();
                let result = self2.detect_with_ner(&text2);
                println!("NER detected {} entities (took {:?})", result.len(), start.elapsed());
                result
            });
            
            let text3 = Arc::clone(&text_arc);
            let self3 = Arc::clone(&self_arc);
            let handle_ai = thread::spawn(move || {
                let start = std::time::Instant::now();
                let result = self3.detect_with_ai(&text3);
                println!("AI detected {} entities (took {:?})", result.len(), start.elapsed());
                result
            });
            
            let text4 = Arc::clone(&text_arc);
            let self4 = Arc::clone(&self_arc);
            let handle_search = thread::spawn(move || {
                let start = std::time::Instant::now();
                let result = self4.detect_with_search(&text4);
                println!("Search detected {} entities (took {:?})", result.len(), start.elapsed());
                result
            });
            
            // 等待所有线程完成并收集结果
            let regex_entities = handle_regex.join().unwrap_or_else(|_| Vec::new());
            let ner_entities = handle_ner.join().unwrap_or_else(|_| Vec::new());
            let ai_entities = handle_ai.join().unwrap_or_else(|_| Vec::new());
            let search_entities = handle_search.join().unwrap_or_else(|_| Vec::new());
            
            // 合并结果：姓名取交集，其他取并集
            let merged = self.merge_detections(
                regex_entities,
                ner_entities,
                ai_entities,
                search_entities,
            );
            
            println!("Final merged: {} entities", merged.len());
            merged
        } else {
            // 传统模式：只使用正则表达式检测
            println!("=== Using regex-only detection (traditional mode) ===");
            
            let regex_entities = self.detect_with_regex(text);
            println!("Regex detected {} entities", regex_entities.len());
            
            regex_entities
        }
    }
    
    /// 批量检测：一次性处理多个文本，显著提升性能
    /// 
    /// # 参数
    /// - texts: 要检测的文本列表
    /// 
    /// # 返回
    /// - Vec<Vec<EntityMatch>>: 每个文本对应的实体列表
    pub fn detect_entities_batch(&self, texts: &[String]) -> Vec<Vec<EntityMatch>> {
        if texts.is_empty() {
            return Vec::new();
        }
        
        println!("=== Batch entity detection for {} texts ===", texts.len());
        
        // 过滤掉不需要检测的文本
        let mut valid_indices = Vec::new();
        let mut valid_texts = Vec::new();
        
        for (idx, text) in texts.iter().enumerate() {
            let trimmed = text.trim();
            
            // 应用相同的过滤逻辑
            if trimmed.is_empty() || trimmed.len() < 5 {
                continue;
            }
            
            let is_numeric_like = trimmed.chars().all(|c| {
                c.is_numeric() || c.is_whitespace() || 
                c == '.' || c == ',' || c == '-' || c == '/' || 
                c == ':' || c == '%' || c == '$' || c == '¥' ||
                c == '(' || c == ')'
            });
            
            if is_numeric_like {
                continue;
            }
            
            let digit_count = trimmed.chars().filter(|c| c.is_numeric()).count();
            let total_count = trimmed.chars().filter(|c| !c.is_whitespace()).count();
            if total_count > 0 && (digit_count as f32 / total_count as f32) > 0.7 {
                continue;
            }
            
            let symbol_chars = trimmed.chars().filter(|c| {
                *c == '├' || *c == '└' || *c == '─' || *c == '│' || 
                *c == '┌' || *c == '┐' || *c == '┘' || *c == '┴' ||
                *c == '：' || *c == '、' || *c == '《' || *c == '》' ||
                *c == '【' || *c == '】' || *c == '（' || *c == '）'
            }).count();
            
            if symbol_chars > 2 {
                continue;
            }
            
            if trimmed.len() < 15 {
                let label_keywords = ["阶段", "步骤", "照片", "图片", "附件", "说明", "备注", "金额", "数量", "单价", "合计"];
                if label_keywords.iter().any(|kw| trimmed.contains(kw)) {
                    continue;
                }
            }
            
            valid_indices.push(idx);
            valid_texts.push(text.clone());
        }
        
        println!("Filtered {} valid texts from {} total", valid_texts.len(), texts.len());
        
        if valid_texts.is_empty() {
            // 所有文本都被过滤了，返回空结果
            return vec![Vec::new(); texts.len()];
        }
        
        // 合并所有有效文本，使用特殊分隔符
        const SEPARATOR: &str = "\n###TEXT_SEPARATOR###\n";
        let combined_text = valid_texts.join(SEPARATOR);
        
        println!("Combined text length: {} chars", combined_text.len());
        
        // 一次性检测所有实体
        let all_entities = if self.use_ai_detection {
            // 使用完整的多方法检测（包括AI）
            self.detect_entities(&combined_text)
        } else {
            // 只使用正则表达式
            self.detect_with_regex(&combined_text)
        };
        
        println!("Detected {} entities in combined text", all_entities.len());
        
        // 将实体映射回各个文本
        let mut results = vec![Vec::new(); texts.len()];
        let mut current_pos = 0;
        
        for (i, (original_idx, text)) in valid_indices.iter().zip(valid_texts.iter()).enumerate() {
            let text_start = current_pos;
            let text_end = text_start + text.len();
            
            // 找到属于当前文本的实体
            for entity in &all_entities {
                if entity.start >= text_start && entity.end <= text_end {
                    // 调整实体位置为相对于原文本的位置
                    let mut text_entity = entity.clone();
                    text_entity.start -= text_start;
                    text_entity.end -= text_start;
                    
                    results[*original_idx].push(text_entity);
                }
            }
            
            // 更新位置（文本长度 + 分隔符长度）
            current_pos = text_end + SEPARATOR.len();
        }
        
        println!("Mapped entities back to {} texts", texts.len());
        
        results
    }
    
    /// 方法1：正则表达式检测
    fn detect_with_regex(&self, text: &str) -> Vec<EntityMatch> {
        let mut entities = Vec::new();
        
        for (entity_type, pattern) in &self.patterns {
            for mat in pattern.find_iter(text) {
                entities.push(EntityMatch {
                    text: mat.as_str().to_string(),
                    entity_type: entity_type.clone(),
                    start: mat.start(),
                    end: mat.end(),
                    confidence: 1.0,
                    source: "regex".to_string(),
                });
            }
        }
        
        entities
    }
    
    /// 方法2：NER 智能检测（基于姓氏和上下文）
    fn detect_with_ner(&self, text: &str) -> Vec<EntityMatch> {
        self.detect_names_smart(text)
    }
    
    /// 方法3：AI 模型检测
    fn detect_with_ai(&self, text: &str) -> Vec<EntityMatch> {
        let mut entities = Vec::new();
        
        // 使用 AI 模型提取所有敏感信息 - 更明确的提示词
        let prompt = format!(
            "从以下文本中提取所有敏感信息，每行一个，格式：类型|内容\n\
            需要提取的类型：\n\
            - 姓名（中文人名）\n\
            - 手机号（11位数字）\n\
            - 邮箱\n\
            - 身份证号\n\
            - 银行卡号\n\
            - IP地址\n\
            - 日期（如2005年9月13日）\n\
            - 地址（如北京市朝阳区、河南省）\n\
            - 地名（如北京城、河南）\n\
            - 组织机构（如某某大学、某某公司）\n\n\
            示例输出：\n\
            姓名|张三\n\
            日期|2005年9月13日\n\
            地址|北京市朝阳区\n\n\
            文本：{}\n\n\
            提取结果（每行一个，必须在原文中出现）：",
            text
        );
        
        println!("Calling AI model for comprehensive entity detection...");
        
        match Self::call_ollama_with_timeout(&prompt, 15) {
            Ok(response) => {
                println!("AI full response:\n{}", response.trim());
                
                // 解析 AI 响应 - 支持两种格式
                for line in response.lines() {
                    let line = line.trim();
                    if line.is_empty() || line == "无" || line.contains("没有") || line.contains("无法") || line.contains("提取结果") {
                        continue;
                    }
                    
                    // 格式1：类型|内容
                    if let Some((type_part, value)) = line.split_once('|') {
                        let entity_type = type_part.trim();
                        let value = value.trim();
                        
                        if !value.is_empty() && value.len() <= 100 {
                            // 在原文中查找位置（必须精确匹配）
                            if let Some(start) = text.find(value) {
                                // 标准化实体类型
                                let normalized_type = Self::normalize_entity_type(entity_type);
                                
                                println!("AI found: {} = '{}' at position {} (normalized: {})", entity_type, value, start, normalized_type);
                                entities.push(EntityMatch {
                                    text: value.to_string(),
                                    entity_type: normalized_type,
                                    start,
                                    end: start + value.len(),
                                    confidence: 0.80,
                                    source: "ai".to_string(),
                                });
                            } else {
                                println!("AI found '{}' (type: {}) but not in original text", value, entity_type);
                            }
                        }
                    }
                    // 格式2：类型：值1,值2
                    else if let Some((type_part, values_part)) = line.split_once('：').or_else(|| line.split_once(':')) {
                        let entity_type = type_part.trim();
                        let values: Vec<&str> = values_part.split(',')
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty() && s.len() <= 100)
                            .collect();
                        
                        for value in values {
                            if let Some(start) = text.find(value) {
                                let normalized_type = Self::normalize_entity_type(entity_type);
                                
                                println!("AI found: {} = '{}' at position {} (normalized: {})", entity_type, value, start, normalized_type);
                                entities.push(EntityMatch {
                                    text: value.to_string(),
                                    entity_type: normalized_type,
                                    start,
                                    end: start + value.len(),
                                    confidence: 0.80,
                                    source: "ai".to_string(),
                                });
                            } else {
                                println!("AI found '{}' (type: {}) but not in original text", value, entity_type);
                            }
                        }
                    }
                }
                
                println!("AI detection extracted {} entities", entities.len());
            },
            Err(e) => {
                println!("AI detection failed or timed out: {}", e);
            }
        }
        
        entities
    }
    
    /// 标准化实体类型名称，统一映射到已知类型
    fn normalize_entity_type(raw_type: &str) -> String {
        match raw_type {
            "姓名" | "人名" | "中文姓名" | "名字" => "姓名".to_string(),
            "手机号" | "电话" | "手机" | "电话号码" => "手机号".to_string(),
            "邮箱" | "电子邮箱" | "email" | "Email" => "邮箱".to_string(),
            "身份证号" | "身份证" | "ID" => "身份证号".to_string(),
            "银行卡号" | "银行卡" | "卡号" => "银行卡号".to_string(),
            "IP地址" | "IP" | "ip" => "IP地址".to_string(),
            "日期" | "时间" | "日期时间" => "日期".to_string(),
            "地址" | "详细地址" | "住址" => "地址".to_string(),
            "地名" | "地点" | "位置" => "地名".to_string(),
            "组织" | "组织机构" | "机构" | "公司" | "单位" | "学校" | "大学" => "组织".to_string(),
            _ => raw_type.to_string(), // 保留原始类型
        }
    }
    
    /// 调用 Ollama 模型（带超时）
    fn call_ollama_with_timeout(prompt: &str, timeout_secs: u64) -> Result<String, String> {
        use std::time::Duration;
        use std::sync::mpsc;
        use std::thread;
        
        let ollama_path = Self::find_ollama_executable()?;
        let prompt = prompt.to_string();
        
        let (tx, rx) = mpsc::channel();
        
        // 在新线程中执行 ollama 命令
        thread::spawn(move || {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                
                let result = Command::new(&ollama_path)
                    .creation_flags(CREATE_NO_WINDOW)
                    .arg("run")
                    .arg("qwen2.5:1.5b")
                    .arg(&prompt)
                    .output();
                
                let _ = tx.send(result);
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                let result = Command::new(&ollama_path)
                    .arg("run")
                    .arg("qwen2.5:1.5b")
                    .arg(&prompt)
                    .output();
                
                let _ = tx.send(result);
            }
        });
        
        // 等待结果或超时
        match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
            Ok(Ok(output)) => {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Err(format!("Ollama failed: {}", String::from_utf8_lossy(&output.stderr)))
                }
            },
            Ok(Err(e)) => Err(format!("Failed to execute ollama: {}", e)),
            Err(_) => Err(format!("Ollama timed out after {} seconds", timeout_secs)),
        }
    }
    
    /// 方法4：字符串搜索匹配
    fn detect_with_search(&self, text: &str) -> Vec<EntityMatch> {
        let mut entities = Vec::new();
        
        // 搜索常见的敏感信息模式
        // 例如：查找类似手机号的数字串、邮箱格式等
        
        // 简单实现：查找11位数字（可能是手机号）
        let chars: Vec<char> = text.chars().collect();
        for i in 0..chars.len() {
            if i + 11 <= chars.len() {
                let candidate: String = chars[i..i+11].iter().collect();
                if candidate.chars().all(|c| c.is_numeric()) && candidate.starts_with('1') {
                    let start_bytes: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
                    let end_bytes = start_bytes + candidate.len();
                    
                    entities.push(EntityMatch {
                        text: candidate,
                        entity_type: "手机号".to_string(),
                        start: start_bytes,
                        end: end_bytes,
                        confidence: 0.7,
                        source: "search".to_string(),
                    });
                }
            }
        }
        
        entities
    }
    
    /// 合并四种检测方法的结果
    fn merge_detections(
        &self,
        regex: Vec<EntityMatch>,
        ner: Vec<EntityMatch>,
        ai: Vec<EntityMatch>,
        search: Vec<EntityMatch>,
    ) -> Vec<EntityMatch> {
        // 按实体文本和位置分组
        let mut entity_groups: HashMap<String, Vec<EntityMatch>> = HashMap::new();
        
        for entity in regex.into_iter().chain(ner).chain(ai).chain(search) {
            let key = format!("{}:{}:{}", entity.text, entity.start, entity.end);
            entity_groups.entry(key).or_insert_with(Vec::new).push(entity);
        }
        
        let mut result = Vec::new();
        
        for (_key, group) in entity_groups {
            if group.is_empty() {
                continue;
            }
            
            let entity_type = &group[0].entity_type;
            
            // 判断是否为姓名类型（支持多种表述）
            let is_name = entity_type == "姓名" || 
                         entity_type == "中文姓名" || 
                         entity_type == "人名" ||
                         entity_type.contains("姓名");
            
            if is_name {
                // 姓名：需要多个检测器确认（至少2个）
                let sources: HashSet<String> = group.iter().map(|e| e.source.clone()).collect();
                
                // 如果启用了 AI，需要至少 2 个检测器确认
                // 如果未启用 AI，只要 NER 检测到就可以（因为 NER 本身已经很严格了）
                let confirmed = if self.use_ai_detection {
                    // AI 模式：需要至少 2 个检测器确认
                    sources.len() >= 2
                } else {
                    // 非 AI 模式：NER 检测到即可
                    sources.contains("ner")
                };
                
                if confirmed {
                    println!("Name '{}' confirmed by {} detectors: {:?}", group[0].text, sources.len(), sources);
                    result.push(EntityMatch {
                        text: group[0].text.clone(),
                        entity_type: "姓名".to_string(), // 统一为"姓名"
                        start: group[0].start,
                        end: group[0].end,
                        confidence: if sources.len() >= 3 { 0.95 } else { 0.85 },
                        source: format!("confirmed_by_{}", sources.len()),
                    });
                } else {
                    println!("Name '{}' not confirmed (only {} detectors: {:?}), skipping", group[0].text, sources.len(), sources);
                }
            } else {
                // 其他类型：任一检测器识别即可（并集）
                // 选择置信度最高的
                let best = group.iter().max_by(|a, b| {
                    a.confidence.partial_cmp(&b.confidence).unwrap()
                }).unwrap();
                
                println!("Entity '{}' (type: {}) detected by {} methods", best.text, best.entity_type, group.len());
                
                result.push(EntityMatch {
                    text: best.text.clone(),
                    entity_type: best.entity_type.clone(),
                    start: best.start,
                    end: best.end,
                    confidence: best.confidence,
                    source: format!("union({})", group.iter().map(|e| e.source.as_str()).collect::<Vec<_>>().join(",")),
                });
            }
        }
        
        // 按位置排序
        result.sort_by_key(|e| e.start);
        result
    }
    
    /// 查找 Ollama 可执行文件
    fn find_ollama_executable() -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            
            // 先检查常见路径（最快，不弹窗）
            let possible_paths = vec![
                format!("{}\\AppData\\Local\\Programs\\Ollama\\ollama.exe", std::env::var("USERPROFILE").unwrap_or_default()),
                "C:\\Program Files\\Ollama\\ollama.exe".to_string(),
                "C:\\Ollama\\ollama.exe".to_string(),
            ];
            
            for path in possible_paths {
                if std::path::Path::new(&path).exists() {
                    return Ok(path);
                }
            }
            
            // 尝试使用 where 命令查找（隐藏窗口）
            use std::os::windows::process::CommandExt;
            if let Ok(output) = Command::new("where")
                .creation_flags(CREATE_NO_WINDOW)
                .arg("ollama")
                .output()
            {
                if output.status.success() {
                    let path_str = String::from_utf8_lossy(&output.stdout);
                    let first_path = path_str.lines().next().unwrap_or("").trim();
                    if !first_path.is_empty() {
                        return Ok(first_path.to_string());
                    }
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // Unix-like 系统
            if let Ok(output) = Command::new("which")
                .arg("ollama")
                .output()
            {
                if output.status.success() {
                    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path_str.is_empty() {
                        return Ok(path_str);
                    }
                }
            }
            
            // 检查常见路径
            let possible_paths = vec![
                "/usr/local/bin/ollama",
                "/usr/bin/ollama",
                "/opt/homebrew/bin/ollama",
            ];
            
            for path in possible_paths {
                if std::path::Path::new(path).exists() {
                    return Ok(path.to_string());
                }
            }
        }
        
        Err("Ollama executable not found".to_string())
    }
    
    // 智能姓名检测：基于常见姓氏和上下文关键词
    fn detect_names_smart(&self, text: &str) -> Vec<EntityMatch> {
        let mut names = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        
        // 常见的非姓名词汇（黑名单）
        let blacklist = vec![
            "方式", "方法", "方面", "方向", "公司", "项目", "系统", "平台",
            "服务", "产品", "技术", "管理", "开发", "设计", "运营", "市场",
            "销售", "客户", "用户", "数据", "信息", "内容", "文件", "文档",
        ];
        
        for i in 0..chars.len() {
            // 检查2-4个汉字的组合
            for len in 2..=4 {
                if i + len > chars.len() {
                    break;
                }
                
                let candidate: String = chars[i..i+len].iter().collect();
                
                // 检查是否全是汉字
                if !candidate.chars().all(|c| c >= '\u{4e00}' && c <= '\u{9fa5}') {
                    continue;
                }
                
                // 检查黑名单
                if blacklist.contains(&candidate.as_str()) {
                    continue;
                }
                
                // 获取第一个字（姓氏）
                let first_char = chars[i].to_string();
                
                // 条件1：第一个字是常见姓氏
                let has_common_surname = self.common_surnames.contains(&first_char);
                
                // 条件2：检查上下文是否包含姓名相关关键词
                let has_context = self.has_name_context(text, i);
                
                // 判断逻辑：
                // - 如果有上下文关键词，姓氏即可
                // - 如果没有上下文，必须是常见姓氏且长度为2-3（典型中文姓名）
                let is_likely_name = if has_context {
                    has_common_surname
                } else {
                    has_common_surname && (len == 2 || len == 3)
                };
                
                if is_likely_name {
                    // 计算在原始文本中的字节位置
                    let start_bytes: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
                    let end_bytes: usize = start_bytes + candidate.len();
                    
                    names.push(EntityMatch {
                        text: candidate.clone(),
                        entity_type: "姓名".to_string(),
                        start: start_bytes,
                        end: end_bytes,
                        confidence: if has_context { 0.85 } else { 0.70 },
                        source: "ner".to_string(),
                    });
                }
            }
        }
        
        names
    }
    
    // 检查姓名候选词周围是否有相关上下文关键词
    fn has_name_context(&self, text: &str, char_pos: usize) -> bool {
        // 获取前后20个字符的上下文
        let chars: Vec<char> = text.chars().collect();
        let start = if char_pos > 20 { char_pos - 20 } else { 0 };
        let end = if char_pos + 20 < chars.len() { char_pos + 20 } else { chars.len() };
        
        let context: String = chars[start..end].iter().collect();
        
        // 检查上下文中是否包含关键词
        for keyword in &self.name_context_keywords {
            if context.contains(keyword) {
                return true;
            }
        }
        
        false
    }
    
    pub fn detect_in_rows(&self, rows: &[Vec<String>]) -> Vec<RowEntities> {
        let mut result = Vec::new();
        
        for (row_index, row) in rows.iter().enumerate() {
            let text = row.join(" ");
            let entities = self.detect_entities(&text);
            
            if !entities.is_empty() {
                result.push(RowEntities {
                    row_index,
                    entities,
                });
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_phone() {
        let detector = NERDetector::new();
        let text = "我的手机号是13812345678";
        let entities = detector.detect_entities(text);
        
        assert!(!entities.is_empty());
        assert_eq!(entities[0].entity_type, "手机号");
        assert_eq!(entities[0].text, "13812345678");
    }
    
    #[test]
    fn test_detect_email() {
        let detector = NERDetector::new();
        let text = "联系邮箱：test@example.com";
        let entities = detector.detect_entities(text);
        
        assert!(!entities.is_empty());
        assert_eq!(entities[0].entity_type, "邮箱");
        assert_eq!(entities[0].text, "test@example.com");
    }
}
