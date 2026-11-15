use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

mod format;
mod mapping;
mod ocr;
mod parser;
pub use format::{FormatCatalog, FormatDefinition, InputFormat, LogicalFormat};
pub use mapping::{
    decode_cmap, decode_server_cmap, encode_server_cmap, encrypt_v2, restore_markdown, CmapVersion,
    MappingError, RestoreResult, ServerCmap, SERVER_CMAP_MAGIC, SERVER_CMAP_VERSION,
};
pub use ocr::{
    ocr_result_to_markdown, validate_ocr_result, OcrPage, OcrQualitySummary, OcrResult,
    OcrTextBlock,
};
pub use parser::{parse_document, parse_input, DocumentFormat, ParsedDocument};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskingRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub replacement_template: String,
    pub enabled: bool,
    pub builtin: bool,
    pub use_counter: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingEntry {
    pub original: String,
    pub masked: String,
    pub rule_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicFinding {
    pub text: String,
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingRequest {
    pub input_format: InputFormat,
    pub content: String,
    pub rules: Vec<MaskingRule>,
    #[serde(default)]
    pub deterministic_findings: Vec<DeterministicFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskingResult {
    pub markdown: String,
    pub mappings: Vec<MappingEntry>,
    pub masked_entity_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilenameMaskingResult {
    pub masked: String,
    pub mappings: Vec<MappingEntry>,
    pub masked_entity_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub safe_details: Option<String>,
}

static BUILTIN_RULES: Lazy<Vec<MaskingRule>> = Lazy::new(|| {
    vec![
        MaskingRule {
            id: "id_card".into(),
            name: "身份证号".into(),
            pattern: r"[1-9]\d{5}(18|19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[\dXx]"
                .into(),
            replacement_template: "***IDCARD***".into(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "phone".into(),
            name: "手机号".into(),
            pattern: r"1[3-9]\d{9}".into(),
            replacement_template: "***PHONE***".into(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "email".into(),
            name: "电子邮箱".into(),
            pattern: r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}".into(),
            replacement_template: "***EMAIL***".into(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "bank_card".into(),
            name: "银行卡号".into(),
            pattern: r"[1-9]\d{15,18}".into(),
            replacement_template: "***BANKCARD***".into(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "ipv4".into(),
            name: "IPv4地址".into(),
            pattern: r"(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)"
                .into(),
            replacement_template: "***IP***".into(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "passport".into(),
            name: "护照号".into(),
            pattern: r"[A-Za-z][0-9]{8}".into(),
            replacement_template: "***PASSPORT***".into(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
        MaskingRule {
            id: "chinese_name".into(),
            name: "中文姓名".into(),
            pattern: r"[一-龥]{2,4}".into(),
            replacement_template: "姓名".into(),
            enabled: true,
            builtin: true,
            use_counter: true,
        },
    ]
});

const COMMON_CHINESE_SURNAMES: &[&str] = &[
    "王", "李", "张", "刘", "陈", "杨", "黄", "赵", "周", "吴", "徐", "孙", "朱", "马", "胡",
    "郭", "林", "何", "高", "梁", "郑", "罗", "宋", "谢", "唐", "韩", "曹", "许", "邓", "萧",
    "冯", "曾", "程", "蔡", "彭", "潘", "袁", "于", "董", "余", "苏", "叶", "吕", "魏", "蒋",
    "田", "杜", "丁", "沈", "姜", "范", "江", "傅", "钟", "卢", "汪", "戴", "崔", "任", "陆",
    "廖", "姚", "方", "金", "邱", "夏", "谭", "韦", "贾", "邹", "石", "熊", "孟", "秦", "阎",
    "薛", "侯", "雷", "白", "龙", "段", "郝", "孔", "邵", "史", "毛", "常", "万", "顾", "赖",
    "武", "康", "贺", "严", "尹", "钱", "施", "牛", "洪", "龚",
];

const COMPOUND_CHINESE_SURNAMES: &[&str] = &[
    "欧阳", "司马", "上官", "诸葛", "东方", "夏侯", "尉迟", "公孙", "慕容", "司徒",
];

pub fn get_builtin_rules() -> &'static Vec<MaskingRule> {
    &BUILTIN_RULES
}

fn filename_name_rule_enabled(rules: &[MaskingRule]) -> bool {
    rules
        .iter()
        .any(|rule| rule.enabled && rule.id == "chinese_name")
}

fn looks_like_filename_name(candidate: &str) -> bool {
    let chars: Vec<char> = candidate.chars().collect();
    if !(2..=4).contains(&chars.len()) {
        return false;
    }

    if !chars.iter().all(|ch| matches!(ch, '\u{4e00}'..='\u{9fa5}')) {
        return false;
    }

    if COMPOUND_CHINESE_SURNAMES.iter().any(|surname| {
        candidate.starts_with(surname)
            && (surname.chars().count() + 1..=surname.chars().count() + 2).contains(&chars.len())
    }) {
        return true;
    }

    COMMON_CHINESE_SURNAMES
        .iter()
        .any(|surname| candidate.starts_with(surname))
}

pub fn collect_filename_findings(value: &str, rules: &[MaskingRule]) -> Vec<DeterministicFinding> {
    if !filename_name_rule_enabled(rules) {
        return vec![];
    }

    let mut findings = Vec::new();
    let mut current_start: Option<usize> = None;

    let push_candidate =
        |findings: &mut Vec<DeterministicFinding>, start: usize, end: usize| {
            let candidate = &value[start..end];
            if looks_like_filename_name(candidate) {
                findings.push(DeterministicFinding {
                    text: candidate.to_string(),
                    entity_type: "姓名".to_string(),
                    start,
                    end,
                });
            }
        };

    for (index, ch) in value.char_indices() {
        if matches!(ch, '\u{4e00}'..='\u{9fa5}') {
            if current_start.is_none() {
                current_start = Some(index);
            }
        } else if let Some(start) = current_start.take() {
            push_candidate(&mut findings, start, index);
        }
    }

    if let Some(start) = current_start {
        push_candidate(&mut findings, start, value.len());
    }

    findings
}

/// A single sensitive-term library entry, in the plain, I/O-free shape the
/// rule constructor below needs. Callers (desktop `commands/masking.rs` and
/// the enterprise Runtime) each own their own persistence and map their
/// stored records into this shape; this crate never reads a database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveTermDefinition {
    pub id: String,
    pub term: String,
    pub category: String,
    pub enabled: bool,
}

/// The single, shared construction of sensitive-term entries into
/// [`MaskingRule`]s. Both the desktop `commands/masking.rs` and the
/// enterprise Runtime's `processing::process_input` must call this — never
/// duplicate the pattern/replacement logic elsewhere (B1/B2).
///
/// Preserves the desktop's pre-existing replacement semantics exactly:
/// - only enabled entries produce a rule;
/// - regex metacharacters in the term are escaped to a literal match;
/// - a term containing any character in `('\u{4E00}', '\u{9FA5}')` (exclusive
///   both ends — the desktop's original boundary, kept as-is rather than
///   "fixed" to an inclusive CJK range) is matched exactly, with no word
///   boundary; all other terms use `\b...\b` word-boundary matching;
/// - replacement is the fixed `[category]` text, never a counted placeholder.
pub fn sensitive_term_rules(terms: &[SensitiveTermDefinition]) -> Vec<MaskingRule> {
    terms
        .iter()
        .filter(|term| term.enabled)
        .map(|term| {
            let escaped_term = regex::escape(&term.term);
            let pattern = if term.term.chars().any(|c| c > '\u{4E00}' && c < '\u{9FA5}') {
                escaped_term
            } else {
                format!(r"\b{}\b", escaped_term)
            };

            MaskingRule {
                id: format!("sensitive_term_{}", term.id),
                name: format!("{} ({})", term.term, term.category),
                pattern,
                replacement_template: format!("[{}]", term.category),
                enabled: true,
                builtin: false,
                use_counter: false,
            }
        })
        .collect()
}

pub struct MaskingService;

impl MaskingService {
    pub fn mask(request: MaskingRequest) -> Result<MaskingResult, AppError> {
        let mut session = MaskingSession::new(request.rules);
        let markdown = session.mask_document(&request.content, &request.deterministic_findings);
        Ok(session.finish(markdown))
    }
}

#[derive(Debug, Clone)]
struct FilenameCandidate {
    start: usize,
    end: usize,
    original: String,
    rule_id: String,
    replacement_template: String,
}

fn has_ascii_digit_neighbor(value: &str, start: usize, end: usize) -> bool {
    let bytes = value.as_bytes();
    let before_is_digit = start > 0 && bytes[start - 1].is_ascii_digit();
    let after_is_digit = end < bytes.len() && bytes[end].is_ascii_digit();
    before_is_digit || after_is_digit
}

fn has_ascii_alnum_neighbor(value: &str, start: usize, end: usize) -> bool {
    let bytes = value.as_bytes();
    let before_is_alnum = start > 0 && bytes[start - 1].is_ascii_alphanumeric();
    let after_is_alnum = end < bytes.len() && bytes[end].is_ascii_alphanumeric();
    before_is_alnum || after_is_alnum
}

fn filename_candidate_priority(rule_id: &str) -> usize {
    if rule_id.starts_with("sensitive_term_") {
        return 0;
    }

    match rule_id {
        "id_card" => 1,
        "bank_card" => 2,
        "phone" => 3,
        "email" => 4,
        "passport" => 5,
        "ipv4" => 6,
        "chinese_name" | "chinese_name_ner" | "name_ner" => 7,
        _ => 8,
    }
}

fn mask_name_for_filename(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 1 {
        return "*".to_string();
    }

    let head = chars[0];
    format!("{head}{}", "*".repeat(chars.len() - 1))
}

fn mask_phone_for_filename(value: &str) -> String {
    if value.chars().count() < 7 {
        return "****".to_string();
    }

    format!("{}****{}", &value[..3], &value[value.len() - 4..])
}

fn mask_id_card_for_filename(value: &str) -> String {
    if value.chars().count() <= 10 {
        return "******".to_string();
    }

    format!(
        "{}{}{}",
        &value[..6],
        "*".repeat(value.len() - 10),
        &value[value.len() - 4..]
    )
}

fn mask_bank_card_for_filename(value: &str) -> String {
    if value.chars().count() <= 8 {
        return "****".to_string();
    }

    format!(
        "{}{}{}",
        &value[..4],
        "*".repeat(value.len() - 8),
        &value[value.len() - 4..]
    )
}

fn mask_passport_for_filename(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 3 {
        return "*".repeat(chars.len().max(1));
    }

    let head = chars[0];
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{head}{}{}", "*".repeat(chars.len() - 3), tail)
}

fn mask_email_for_filename(value: &str) -> String {
    let Some((local, domain)) = value.split_once('@') else {
        return "***EMAIL***".to_string();
    };

    let local_head = local.chars().next().unwrap_or('*');
    format!("{local_head}***@{domain}")
}

fn mask_ipv4_for_filename(value: &str) -> String {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() == 4 {
        format!("{}.{}.*.*", parts[0], parts[1])
    } else {
        "***IP***".to_string()
    }
}

fn mask_filename_value(rule_id: &str, original: &str, replacement_template: &str) -> String {
    match rule_id {
        "phone" | "phone_ner" => mask_phone_for_filename(original),
        "id_card" | "id_card_ner" => mask_id_card_for_filename(original),
        "bank_card" | "bank_card_ner" => mask_bank_card_for_filename(original),
        "passport" | "passport_ner" => mask_passport_for_filename(original),
        "email" | "email_ner" => mask_email_for_filename(original),
        "ipv4" | "ipv4_ner" => mask_ipv4_for_filename(original),
        "chinese_name" | "chinese_name_ner" | "name_ner" => mask_name_for_filename(original),
        _ if rule_id.starts_with("sensitive_term_") => replacement_template.to_string(),
        _ => replacement_template.to_string(),
    }
}

pub fn mask_filename(
    value: &str,
    rules: &[MaskingRule],
    deterministic_findings: &[DeterministicFinding],
) -> FilenameMaskingResult {
    let mut candidates = Vec::new();

    for rule in rules {
        if !rule.enabled || rule.id == "chinese_name" {
            continue;
        }

        let regex = match Regex::new(&rule.pattern) {
            Ok(regex) => regex,
            Err(_) => continue,
        };

        for capture in regex.find_iter(value) {
            if matches!(rule.id.as_str(), "phone" | "id_card" | "bank_card")
                && has_ascii_digit_neighbor(value, capture.start(), capture.end())
            {
                continue;
            }
            if rule.id == "passport"
                && has_ascii_alnum_neighbor(value, capture.start(), capture.end())
            {
                continue;
            }

            candidates.push(FilenameCandidate {
                start: capture.start(),
                end: capture.end(),
                original: capture.as_str().to_string(),
                rule_id: rule.id.clone(),
                replacement_template: rule.replacement_template.clone(),
            });
        }
    }

    for finding in deterministic_findings {
        if finding.text.is_empty() || finding.end <= finding.start || finding.end > value.len() {
            continue;
        }

        let rule_id = match finding.entity_type.as_str() {
            "姓名" | "中文姓名" => "chinese_name",
            "手机号" => "phone",
            "身份证号" => "id_card",
            "银行卡号" => "bank_card",
            "邮箱" => "email",
            "IP地址" => "ipv4",
            "护照号" => "passport",
            _ => continue,
        };

        let Some(rule) = rules.iter().find(|rule| rule.enabled && rule.id == rule_id) else {
            continue;
        };

        candidates.push(FilenameCandidate {
            start: finding.start,
            end: finding.end,
            original: finding.text.clone(),
            rule_id: rule.id.clone(),
            replacement_template: rule.replacement_template.clone(),
        });
    }

    candidates.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| filename_candidate_priority(&left.rule_id).cmp(&filename_candidate_priority(&right.rule_id)))
            .then_with(|| (right.end - right.start).cmp(&(left.end - left.start)))
    });

    let mut accepted = Vec::new();
    let mut last_end = 0usize;
    for candidate in candidates {
        if candidate.start < last_end {
            continue;
        }
        last_end = candidate.end;
        accepted.push(candidate);
    }

    let mut result = String::new();
    let mut cursor = 0usize;
    let mut mappings = Vec::new();

    for candidate in &accepted {
        if candidate.start > cursor {
            result.push_str(&value[cursor..candidate.start]);
        }

        let masked = mappings
            .iter()
            .find(|entry: &&MappingEntry| {
                entry.original == candidate.original && entry.rule_id == candidate.rule_id
            })
            .map(|entry| entry.masked.clone())
            .unwrap_or_else(|| {
                let masked = mask_filename_value(
                    &candidate.rule_id,
                    &candidate.original,
                    &candidate.replacement_template,
                );
                mappings.push(MappingEntry {
                    original: candidate.original.clone(),
                    masked: masked.clone(),
                    rule_id: candidate.rule_id.clone(),
                });
                masked
            });

        result.push_str(&masked);
        cursor = candidate.end;
    }

    if cursor < value.len() {
        result.push_str(&value[cursor..]);
    }

    FilenameMaskingResult {
        masked: if accepted.is_empty() { value.to_string() } else { result },
        mappings,
        masked_entity_count: accepted.len(),
    }
}

pub struct MaskingSession {
    rules: Vec<MaskingRule>,
    mappings: Vec<MappingEntry>,
    placeholder_counter: usize,
    masked_entity_count: usize,
    warnings: Vec<String>,
}

impl MaskingSession {
    pub fn new(rules: Vec<MaskingRule>) -> Self {
        Self::with_state(rules, Vec::new(), 0)
    }

    pub fn with_state(
        rules: Vec<MaskingRule>,
        mappings: Vec<MappingEntry>,
        placeholder_counter: usize,
    ) -> Self {
        Self {
            rules,
            mappings,
            placeholder_counter,
            masked_entity_count: 0,
            warnings: Vec::new(),
        }
    }

    pub fn mask_fragment(&mut self, value: &str, findings: &[DeterministicFinding]) -> String {
        let mut result = value.to_string();

        for rule in self.rules.clone() {
            if !rule.enabled {
                continue;
            }
            let regex = match Regex::new(&rule.pattern) {
                Ok(regex) => regex,
                Err(_) => {
                    self.warnings
                        .push(format!("INVALID_RULE_PATTERN:{}", rule.id));
                    continue;
                }
            };

            result = regex
                .replace_all(&result, |captures: &regex::Captures| {
                    let original = captures[0].to_string();
                    self.masked_entity_count += 1;
                    if let Some(entry) = self
                        .mappings
                        .iter()
                        .find(|entry| entry.original == original)
                    {
                        return entry.masked.clone();
                    }

                    let masked = if rule.use_counter {
                        self.placeholder_counter += 1;
                        format!("{}{}", rule.replacement_template, self.placeholder_counter)
                    } else {
                        rule.replacement_template.clone()
                    };
                    self.mappings.push(MappingEntry {
                        original,
                        masked: masked.clone(),
                        rule_id: rule.id.clone(),
                    });
                    masked
                })
                .into_owned();
        }

        self.apply_findings(result, findings, true, false)
    }

    /// Internal: mask one line using pre-compiled rule data (regex + owned
    /// field copies).  Avoids re-borrowing self.rules inside the replace_all
    /// closure, making the borrow checker happy.
    fn mask_line_fast(
        &mut self,
        value: &str,
        compiled: &[(regex::Regex, bool, String, String)],
        findings: &[DeterministicFinding],
    ) -> String {
        let mut result = value.to_string();
        for (regex, use_counter, template, rule_id) in compiled {
            result = regex
                .replace_all(&result, |captures: &regex::Captures| {
                    let original = captures[0].to_string();
                    self.masked_entity_count += 1;
                    if let Some(entry) = self
                        .mappings
                        .iter()
                        .find(|entry| entry.original == original)
                    {
                        return entry.masked.clone();
                    }
                    let masked = if *use_counter {
                        self.placeholder_counter += 1;
                        format!("{}{}", template, self.placeholder_counter)
                    } else {
                        template.clone()
                    };
                    self.mappings.push(MappingEntry {
                        original,
                        masked: masked.clone(),
                        rule_id: rule_id.clone(),
                    });
                    masked
                })
                .into_owned();
        }
        self.apply_findings(result, findings, true, false)
    }

    /// Process a document by line, pre-compiling regexes once for all lines.
    pub fn mask_document(&mut self, content: &str, findings: &[DeterministicFinding]) -> String {
        // Pre-compile regexes + copy rule fields into owned data once, not per
        // line.  This avoids O(lines × rules) regex compilation – formerly the
        // dominant cost for large inputs (10 MB → ~230k lines × 2 regexes).
        let compiled: Vec<(regex::Regex, bool, String, String)> = self
            .rules
            .iter()
            .filter(|r| r.enabled)
            .filter(|r| r.id != "chinese_name")
            .filter_map(|rule| {
                regex::Regex::new(&rule.pattern).ok().map(|re| {
                    (
                        re,
                        rule.use_counter,
                        rule.replacement_template.clone(),
                        rule.id.clone(),
                    )
                })
            })
            .collect();
        // Report compilation failures without holding a self.rules iterator.
        for rule in &self.rules {
            if rule.enabled && regex::Regex::new(&rule.pattern).is_err() {
                self.warnings
                    .push(format!("INVALID_RULE_PATTERN:{}", rule.id));
            }
        }
        content
            .split_inclusive('\n')
            .map(|line| self.mask_line_fast(line, &compiled, findings))
            .collect()
    }

    pub fn apply_findings_fragment_unchecked(
        &mut self,
        value: &str,
        findings: &[DeterministicFinding],
    ) -> String {
        self.apply_findings(value.to_string(), findings, false, true)
    }

    fn apply_findings(
        &mut self,
        mut result: String,
        findings: &[DeterministicFinding],
        enforce_rule_switches: bool,
        legacy_compatibility: bool,
    ) -> String {
        for finding in findings {
            if finding.text.is_empty() || !result.contains(&finding.text) {
                continue;
            }
            let occurrences = result.matches(&finding.text).count();
            if occurrences == 0 {
                continue;
            }

            let enabled_rule_ids: std::collections::HashSet<&str> = self
                .rules
                .iter()
                .filter(|rule| rule.enabled)
                .map(|rule| rule.id.as_str())
                .collect();
            let (prefix, suffix, rule_id, requires_rule) =
                finding_mask(&finding.entity_type, legacy_compatibility);
            if enforce_rule_switches
                && requires_rule
                    .is_some_and(|required_rule| !enabled_rule_ids.contains(required_rule))
            {
                continue;
            }

            self.masked_entity_count += occurrences;
            let masked = if let Some(entry) = self
                .mappings
                .iter()
                .find(|entry| entry.original == finding.text)
            {
                entry.masked.clone()
            } else {
                self.placeholder_counter += 1;
                let masked = format!("{}{}{}", prefix, self.placeholder_counter, suffix);
                self.mappings.push(MappingEntry {
                    original: finding.text.clone(),
                    masked: masked.clone(),
                    rule_id: rule_id.into(),
                });
                masked
            };
            result = result.replace(&finding.text, &masked);
        }

        result
    }

    pub fn mappings(&self) -> &[MappingEntry] {
        &self.mappings
    }

    pub fn placeholder_counter(&self) -> usize {
        self.placeholder_counter
    }

    pub fn masked_entity_count(&self) -> usize {
        self.masked_entity_count
    }

    pub fn finish(self, markdown: String) -> MaskingResult {
        MaskingResult {
            markdown,
            mappings: self.mappings,
            masked_entity_count: self.masked_entity_count,
            warnings: self.warnings,
        }
    }
}

fn finding_mask(
    entity_type: &str,
    legacy_compatibility: bool,
) -> (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
) {
    if legacy_compatibility {
        return match entity_type {
            "身份证号" => ("***IDCARD", "***", "id_card_ner", Some("id_card")),
            "手机号" => ("***PHONE", "***", "phone_ner", Some("phone")),
            "邮箱" => ("***EMAIL", "***", "email_ner", Some("email")),
            "银行卡号" => ("***BANKCARD", "***", "bank_card_ner", Some("bank_card")),
            "IP地址" => ("***IP", "***", "ipv4_ner", Some("ipv4")),
            "护照号" => ("***PASSPORT", "***", "passport_ner", Some("passport")),
            "姓名" => ("***NAME", "***", "name_ner", Some("chinese_name")),
            _ => ("***SENSITIVE", "***", "unknown_ner", None),
        };
    }

    match entity_type {
        "身份证号" => ("***IDCARD", "***", "id_card_ner", Some("id_card")),
        "手机号" => ("***PHONE", "***", "phone_ner", Some("phone")),
        "邮箱" => ("***EMAIL", "***", "email_ner", Some("email")),
        "银行卡号" => ("***BANKCARD", "***", "bank_card_ner", Some("bank_card")),
        "IP地址" => ("***IP", "***", "ipv4_ner", Some("ipv4")),
        "护照号" => ("***PASSPORT", "***", "passport_ner", Some("passport")),
        "姓名" | "中文姓名" => ("姓名", "", "chinese_name_ner", Some("chinese_name")),
        "日期" => ("***DATE", "***", "date_ner", None),
        "地址" => ("***ADDRESS", "***", "address_ner", None),
        "地名" => ("***LOCATION", "***", "location_ner", None),
        "组织" => ("***ORG", "***", "organization_ner", None),
        _ => ("***SENSITIVE", "***", "unknown_ner", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(ids: &[&str]) -> Vec<MaskingRule> {
        get_builtin_rules()
            .iter()
            .filter(|rule| ids.contains(&rule.id.as_str()))
            .cloned()
            .map(|mut rule| {
                rule.enabled = true;
                rule
            })
            .collect()
    }

    #[test]
    fn no_match_returns_original_and_zero_count() {
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "普通合成文本".into(),
            rules: rules(&["phone"]),
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.markdown, "普通合成文本");
        assert!(result.mappings.is_empty());
        assert_eq!(result.masked_entity_count, 0);
    }

    #[test]
    fn disabled_rule_does_not_mask() {
        let mut phone_rule = rules(&["phone"]).remove(0);
        phone_rule.enabled = false;
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "13900000000".into(),
            rules: vec![phone_rule],
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.markdown, "13900000000");
        assert_eq!(result.masked_entity_count, 0);
    }

    #[test]
    fn masks_phone_email_and_id_card() {
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "13900000000 unit.test@example.invalid 110105199901011234".into(),
            rules: rules(&["phone", "email", "id_card"]),
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.masked_entity_count, 3);
        assert_eq!(result.mappings.len(), 3);
    }

    #[test]
    fn repeated_value_reuses_placeholder_but_counts_occurrences() {
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "13900000000 / 13900000000".into(),
            rules: rules(&["phone"]),
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.masked_entity_count, 2);
        assert_eq!(result.mappings.len(), 1);
        assert_eq!(result.markdown.matches("***PHONE***1").count(), 2);
    }

    #[test]
    fn different_values_have_independent_mappings() {
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "13900000000 13800000000".into(),
            rules: rules(&["phone"]),
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.masked_entity_count, 2);
        assert_eq!(result.mappings.len(), 2);
        assert_ne!(result.mappings[0].masked, result.mappings[1].masked);
    }

    #[test]
    fn output_is_stable_and_markdown_structure_is_preserved() {
        let request = MaskingRequest {
            input_format: InputFormat::Markdown,
            content: "# 标题\n\n- 联系：13900000000\n".into(),
            rules: rules(&["phone"]),
            deterministic_findings: vec![],
        };
        let first = MaskingService::mask(request.clone()).unwrap();
        let second = MaskingService::mask(request).unwrap();
        assert_eq!(first, second);
        assert!(first.markdown.starts_with("# 标题\n\n- 联系："));
        assert!(first.markdown.ends_with('\n'));
    }

    #[test]
    #[ignore = "19-min benchmark; run with: cargo test --release perf_10mb_text_20_samples -- --nocapture --ignored"]
    fn perf_10mb_text_20_samples() {
        use std::time::Instant;

        let text = "Call 13900000000 and email test@example.com\n".repeat(230_000);
        assert!(
            text.len() > 9_500_000,
            "text size: {} bytes (<10 MB)",
            text.len()
        );
        let rules = get_builtin_rules()
            .iter()
            .filter(|r| ["phone", "email"].contains(&r.id.as_str()))
            .cloned()
            .map(|mut r| {
                r.enabled = true;
                r
            })
            .collect::<Vec<_>>();
        let mut times = Vec::with_capacity(20);
        for i in 0..20 {
            let start = Instant::now();
            let result = MaskingService::mask(MaskingRequest {
                input_format: InputFormat::Text,
                content: text.clone(),
                rules: rules.clone(),
                deterministic_findings: vec![],
            })
            .unwrap();
            let elapsed = start.elapsed().as_secs_f64();
            times.push(elapsed);
            assert!(
                result.masked_entity_count > 0,
                "sample {i}: no entities masked"
            );
            eprintln!("  sample {:2}: {:.3}s", i + 1, elapsed);
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = times[9];
        let p95 = times[18];
        let max = times.last().unwrap();
        let os = std::env::consts::OS;
        println!("\n=== 10 MB Text Masking Performance ===");
        println!("Environment: {} — NOT Windows baseline", os);
        println!("Samples:     20");
        println!("P50:         {p50:.3}s");
        println!("P95:         {p95:.3}s");
        println!("Max:         {max:.3}s");
        println!("Failures:    0");
        println!("PRD threshold (Windows 10/11 x64, 8C, 16 GB, SSD): P95 ≤ 5s");
        let os = std::env::consts::OS;
        if os == "windows" {
            assert!(
                p95 <= 5.0,
                "FAIL: P95={p95:.3}s exceeds PRD threshold of 5s on Windows"
            );
            println!("RESULT: **PASS** (on Windows baseline)");
        } else {
            println!("RESULT: **REFERENCE ONLY** (on {os}, not Windows baseline)");
            println!("NOTE:  P95={p95:.3}s (reference); PRD threshold is 5s on Windows.");
            println!("       This is a non-baseline environment ({os}).  Re-run on Windows");
            println!("       10/11 x64, 8C, 16 GB, SSD for a binding result.");
        }
    }

    fn sensitive_term(
        id: &str,
        term: &str,
        category: &str,
        enabled: bool,
    ) -> SensitiveTermDefinition {
        SensitiveTermDefinition {
            id: id.into(),
            term: term.into(),
            category: category.into(),
            enabled,
        }
    }

    #[test]
    fn builtin_chinese_name_is_enabled_by_default() {
        let rule = get_builtin_rules()
            .iter()
            .find(|rule| rule.id == "chinese_name")
            .expect("builtin chinese_name rule should exist");

        assert!(rule.enabled);
        assert_eq!(rule.replacement_template, "姓名");
    }

    #[test]
    fn enabled_chinese_name_does_not_regex_mask_plain_text_without_findings() {
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "项目代号：流程验证\n备注：完成验收".into(),
            rules: rules(&["chinese_name"]),
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.markdown, "项目代号：流程验证\n备注：完成验收");
        assert_eq!(result.masked_entity_count, 0);
    }

    #[test]
    fn deterministic_name_findings_mask_when_chinese_name_enabled() {
        let content = "客户姓名：张三\n联系电话：13900000000\n";
        let start = content.find("张三").unwrap();
        let end = start + "张三".len();
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: content.into(),
            rules: rules(&["chinese_name", "phone"]),
            deterministic_findings: vec![DeterministicFinding {
                text: "张三".into(),
                entity_type: "姓名".into(),
                start,
                end,
            }],
        })
        .unwrap();

        assert!(result.markdown.contains("客户姓名：姓名1"));
        assert!(result.markdown.contains("联系电话：***PHONE***2"));
        assert!(!result.markdown.contains("客户姓名：张三"));
        assert_eq!(result.masked_entity_count, 2);
    }

    #[test]
    fn mask_filename_applies_compliant_partial_masks() {
        let rules = rules(&["chinese_name", "phone", "id_card", "bank_card"]);
        let findings = vec![DeterministicFinding {
            text: "张三".into(),
            entity_type: "姓名".into(),
            start: 0,
            end: "张三".len(),
        }];

        let result = mask_filename(
            "张三_13812345678_11010519491231002X_6222020202020202020_合同",
            &rules,
            &findings,
        );

        assert_eq!(
            result.masked,
            "张*_138****5678_110105********002X_6222***********2020_合同"
        );
        assert_eq!(result.masked_entity_count, 4);
    }

    #[test]
    fn mask_filename_uses_category_for_sensitive_terms() {
        let rules = sensitive_term_rules(&[sensitive_term("t1", "机密项目", "机密", true)]);
        let result = mask_filename("机密项目-交付件", &rules, &[]);
        assert_eq!(result.masked, "[机密]-交付件");
        assert_eq!(result.masked_entity_count, 1);
    }

    #[test]
    fn mask_filename_supports_legacy_15_digit_id_cards() {
        let result = mask_filename("档案_130503670401001_签收", &rules(&["id_card"]), &[]);
        assert_eq!(result.masked, "档案_130503*****1001_签收");
        assert_eq!(result.masked_entity_count, 1);
    }

    #[test]
    fn mask_filename_handles_adjacent_sensitive_segments_without_separators() {
        let findings = vec![DeterministicFinding {
            text: "张三".into(),
            entity_type: "姓名".into(),
            start: 0,
            end: "张三".len(),
        }];
        let result = mask_filename(
            "张三13812345678合同",
            &rules(&["chinese_name", "phone"]),
            &findings,
        );
        assert_eq!(result.masked, "张*138****5678合同");
        assert_eq!(result.masked_entity_count, 2);
    }

    #[test]
    fn mask_filename_skips_phone_inside_bank_card_overlap() {
        let result = mask_filename("6222021381234567890", &rules(&["phone", "bank_card"]), &[]);
        assert_eq!(result.masked, "6222***********7890");
        assert_eq!(result.masked_entity_count, 1);
    }

    /// B3: a term containing CJK characters is matched exactly, with no word
    /// boundary — mirroring the desktop's original character-range check.
    #[test]
    fn sensitive_term_rules_matches_chinese_terms_exactly_without_word_boundary() {
        let rules = sensitive_term_rules(&[sensitive_term("t1", "张三", "姓名", true)]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "张三");
        assert_eq!(rules[0].replacement_template, "[姓名]");
        assert!(!rules[0].use_counter);
        assert!(rules[0].enabled);
        assert!(!rules[0].builtin);

        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "联系人张三先生，另有张三丰".into(),
            rules,
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.markdown, "联系人[姓名]先生，另有[姓名]丰");
        assert_eq!(result.masked_entity_count, 2);
    }

    /// B3: a pure English/digit term keeps existing word-boundary semantics
    /// — "CEO" matches as a whole word but not inside "CEOs".
    #[test]
    fn sensitive_term_rules_matches_english_terms_with_word_boundary() {
        let rules = sensitive_term_rules(&[sensitive_term("t2", "CEO", "职位", true)]);
        assert_eq!(rules[0].pattern, r"\bCEO\b");

        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "our CEO said; CEOs elsewhere agree".into(),
            rules,
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.markdown, "our [职位] said; CEOs elsewhere agree");
        assert_eq!(result.masked_entity_count, 1);
    }

    /// B2/安全约束5: regex metacharacters in a term are escaped to a literal
    /// match, never interpreted as a pattern — prevents regex injection.
    #[test]
    fn sensitive_term_rules_escapes_regex_metacharacters() {
        let rules = sensitive_term_rules(&[sensitive_term("t3", "a.b*c", "内部代号", true)]);
        let result = MaskingService::mask(MaskingRequest {
            input_format: InputFormat::Text,
            content: "code a.b*c here, but not axbyyc".into(),
            rules,
            deterministic_findings: vec![],
        })
        .unwrap();
        assert_eq!(result.markdown, "code [内部代号] here, but not axbyyc");
        assert_eq!(result.masked_entity_count, 1);
    }

    /// 只转换启用词条: disabled entries produce no rule at all.
    #[test]
    fn sensitive_term_rules_skips_disabled_terms() {
        let rules = sensitive_term_rules(&[
            sensitive_term("t4", "启用词", "分类", true),
            sensitive_term("t5", "禁用词", "分类", false),
        ]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "sensitive_term_t4");
    }

    #[test]
    fn sensitive_term_rules_returns_empty_for_empty_input() {
        assert!(sensitive_term_rules(&[]).is_empty());
    }
}
