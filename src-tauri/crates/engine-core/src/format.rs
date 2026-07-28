use serde::{Deserialize, Serialize};

use crate::AppError;

/// The input formats that currently have a parser and can enter the
/// enterprise pipeline. This remains separate from `LogicalFormat`, which
/// also describes future format families without pretending they are parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputFormat {
    Text,
    Markdown,
    Csv,
    Excel,
    Docx,
    Pdf,
    Powerpoint,
}

impl InputFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Csv => "csv",
            Self::Excel => "excel",
            Self::Docx => "docx",
            Self::Pdf => "pdf",
            Self::Powerpoint => "powerpoint",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "markdown" => Some(Self::Markdown),
            "csv" => Some(Self::Csv),
            "excel" => Some(Self::Excel),
            "docx" => Some(Self::Docx),
            "pdf" => Some(Self::Pdf),
            "powerpoint" => Some(Self::Powerpoint),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalFormat {
    Text,
    Markdown,
    Csv,
    Excel,
    Word,
    Pdf,
    Powerpoint,
}

impl LogicalFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Csv => "csv",
            Self::Excel => "excel",
            Self::Word => "word",
            Self::Pdf => "pdf",
            Self::Powerpoint => "powerpoint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatDefinition {
    pub extension: &'static str,
    pub logical_format: LogicalFormat,
    pub input_format: Option<InputFormat>,
    pub enterprise_supported: bool,
    pub parser_supported: bool,
    pub output_extension: &'static str,
}

impl FormatDefinition {
    pub const fn is_currently_parsed(self) -> bool {
        self.parser_supported && self.input_format.is_some()
    }
}

pub struct FormatCatalog;

impl FormatCatalog {
    /// This is the only format matrix used by the shared Rust boundary.
    /// Future families are catalogued but are intentionally not enterprise
    /// upload formats until their dedicated parser task is approved.
    pub const DEFINITIONS: &'static [FormatDefinition] = &[
        FormatDefinition {
            extension: "txt",
            logical_format: LogicalFormat::Text,
            input_format: Some(InputFormat::Text),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "md",
            logical_format: LogicalFormat::Markdown,
            input_format: Some(InputFormat::Markdown),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "markdown",
            logical_format: LogicalFormat::Markdown,
            input_format: Some(InputFormat::Markdown),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "csv",
            logical_format: LogicalFormat::Csv,
            input_format: Some(InputFormat::Csv),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "xls",
            logical_format: LogicalFormat::Excel,
            input_format: Some(InputFormat::Excel),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "xlsx",
            logical_format: LogicalFormat::Excel,
            input_format: Some(InputFormat::Excel),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "doc",
            logical_format: LogicalFormat::Word,
            input_format: None,
            enterprise_supported: false,
            parser_supported: false,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "docx",
            logical_format: LogicalFormat::Word,
            input_format: Some(InputFormat::Docx),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "pdf",
            logical_format: LogicalFormat::Pdf,
            input_format: Some(InputFormat::Pdf),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "ppt",
            logical_format: LogicalFormat::Powerpoint,
            input_format: Some(InputFormat::Powerpoint),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
        FormatDefinition {
            extension: "pptx",
            logical_format: LogicalFormat::Powerpoint,
            input_format: Some(InputFormat::Powerpoint),
            enterprise_supported: true,
            parser_supported: true,
            output_extension: "md",
        },
    ];

    pub const fn all() -> &'static [FormatDefinition] {
        Self::DEFINITIONS
    }

    pub fn from_filename(filename: &str) -> Option<FormatDefinition> {
        let basename = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
        let separator = basename.rfind('.')?;
        if separator == 0 {
            return None;
        }
        let extension = basename[separator + 1..].to_ascii_lowercase();
        Self::DEFINITIONS
            .iter()
            .copied()
            .find(|definition| definition.extension == extension)
    }

    pub fn enterprise_from_filename(filename: &str) -> Result<FormatDefinition, AppError> {
        let definition = Self::from_filename(filename).ok_or_else(|| {
            unsupported_format_error("The input format is not supported")
        })?;
        if !definition.enterprise_supported || !definition.is_currently_parsed() {
            return Err(unsupported_format_error(
                "The input format is not supported by the enterprise runtime",
            ));
        }
        Ok(definition)
    }
}

fn unsupported_format_error(message: &str) -> AppError {
    AppError {
        code: "INPUT_FORMAT_UNSUPPORTED".to_string(),
        message: message.to_string(),
        retryable: false,
        safe_details: None,
    }
}
