use regex::Regex;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub text: String,
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowEntities {
    pub row_index: usize,
    pub entities: Vec<EntityMatch>,
}

pub struct NERDetector {
    patterns: Vec<(String, Regex)>,
    common_surnames: HashSet<String>,
    name_context_keywords: Vec<String>,
}

impl NERDetector {
    pub fn new() -> Self {
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
        
        // 日期格式（优先于中文姓名检测）
        patterns.push((
            "日期".to_string(),
            Regex::new(r"\b\d{4}[-/年]\d{1,2}[-/月]\d{1,2}[日]?\b").unwrap()
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
        }
    }
    
    pub fn detect_entities(&self, text: &str) -> Vec<EntityMatch> {
        let mut entities = Vec::new();
        
        // 先检测其他类型的实体
        for (entity_type, pattern) in &self.patterns {
            for mat in pattern.find_iter(text) {
                entities.push(EntityMatch {
                    text: mat.as_str().to_string(),
                    entity_type: entity_type.clone(),
                    start: mat.start(),
                    end: mat.end(),
                });
            }
        }
        
        // 智能检测姓名（基于姓氏和上下文）
        let name_entities = self.detect_names_smart(text);
        entities.extend(name_entities);
        
        // 按位置排序
        entities.sort_by_key(|e| (e.start, e.end));
        
        // 去重：如果两个实体有重叠，保留更具体的（非"姓名"类型优先）
        let mut filtered = Vec::new();
        for entity in entities {
            let overlaps = filtered.iter().any(|e: &EntityMatch| {
                // 检查是否有重叠
                (entity.start >= e.start && entity.start < e.end) ||
                (entity.end > e.start && entity.end <= e.end) ||
                (entity.start <= e.start && entity.end >= e.end)
            });
            
            if !overlaps {
                filtered.push(entity);
            } else if entity.entity_type != "姓名" {
                // 如果是更具体的类型，替换掉姓名类型
                filtered.retain(|e| {
                    !(e.entity_type == "姓名" && 
                      ((entity.start >= e.start && entity.start < e.end) ||
                       (entity.end > e.start && entity.end <= e.end) ||
                       (entity.start <= e.start && entity.end >= e.end)))
                });
                filtered.push(entity);
            }
        }
        
        // 最终排序
        filtered.sort_by_key(|e| e.start);
        filtered
    }
    
    // 智能姓名检测：基于常见姓氏和上下文关键词
    fn detect_names_smart(&self, text: &str) -> Vec<EntityMatch> {
        let mut names = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        
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
                
                // 获取第一个字（姓氏）
                let first_char = chars[i].to_string();
                
                // 条件1：第一个字是常见姓氏
                let has_common_surname = self.common_surnames.contains(&first_char);
                
                // 条件2：检查上下文是否包含姓名相关关键词
                let has_context = self.has_name_context(text, i);
                
                // 如果满足姓氏条件或上下文条件，则认为是姓名
                if has_common_surname || has_context {
                    // 计算在原始文本中的字节位置
                    let start_bytes: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
                    let end_bytes: usize = start_bytes + candidate.len();
                    
                    names.push(EntityMatch {
                        text: candidate.clone(),
                        entity_type: "姓名".to_string(),
                        start: start_bytes,
                        end: end_bytes,
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
