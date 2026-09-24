//! Per-compilation options: `sys.inputs` and PDF export settings.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroUsize;
use std::str::FromStr;

/// Options for [`compile_with_options`](crate::compile_with_options).
///
/// Unlike [`Config`](crate::Config), which describes a project (template and
/// font directories), these options describe a single compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOptions {
    /// String values visible to the document as `sys.inputs`.
    pub inputs: BTreeMap<String, String>,
    /// PDF standards Typst enforces, for example PDF/A-2b. Typst fails the
    /// compilation if the document cannot conform. Empty means plain PDF.
    /// PDF/A needs a document date: set
    /// [`creation_timestamp`](Self::creation_timestamp) or
    /// `#set document(date: ..)` in the document.
    pub pdf_standards: Vec<PdfStandard>,
    /// Creation time as a Unix timestamp (seconds, UTC). It is written to the
    /// PDF metadata (unless the document sets its own date) and used as the
    /// current date for `datetime.today()`, which makes output reproducible.
    /// `None` writes no creation date and uses the system clock for `today()`.
    pub creation_timestamp: Option<i64>,
    /// Pages to export. Empty exports all pages. Typst cannot tag a partial
    /// export, so a non-empty list always produces an untagged PDF.
    pub pages: Vec<PageRange>,
    /// Write a tagged PDF (the Typst default), which gives assistive
    /// technology the document structure. Turning this off makes the file
    /// smaller. Required by PDF/UA-1 and PDF/A level `a`. Ignored (treated
    /// as `false`) when [`pages`](Self::pages) is set.
    pub pdf_tags: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            inputs: BTreeMap::new(),
            pdf_standards: Vec::new(),
            creation_timestamp: None,
            pages: Vec::new(),
            pdf_tags: true,
        }
    }
}

macro_rules! pdf_standards {
    ($($variant:ident => $name:literal, $typst:ident, $doc:literal;)*) => {
        /// A PDF version or standard Typst can enforce.
        ///
        /// Parses from and displays as the names the `typst` CLI uses, for
        /// example `1.7`, `a-2b` or `ua-1`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum PdfStandard {
            $(#[doc = $doc] $variant,)*
        }

        impl PdfStandard {
            /// Every supported standard, in the order the `typst` CLI lists them.
            pub const ALL: &'static [PdfStandard] = &[$(PdfStandard::$variant,)*];

            /// The name used on the command line, for example `a-2b`.
            pub fn name(self) -> &'static str {
                match self {
                    $(PdfStandard::$variant => $name,)*
                }
            }

            pub(crate) fn to_typst(self) -> typst_pdf::PdfStandard {
                match self {
                    $(PdfStandard::$variant => typst_pdf::PdfStandard::$typst,)*
                }
            }
        }
    };
}

pdf_standards! {
    V1_4 => "1.4", V_1_4, "PDF 1.4.";
    V1_5 => "1.5", V_1_5, "PDF 1.5.";
    V1_6 => "1.6", V_1_6, "PDF 1.6.";
    V1_7 => "1.7", V_1_7, "PDF 1.7.";
    V2_0 => "2.0", V_2_0, "PDF 2.0.";
    A1b => "a-1b", A_1b, "PDF/A-1b.";
    A1a => "a-1a", A_1a, "PDF/A-1a.";
    A2b => "a-2b", A_2b, "PDF/A-2b.";
    A2u => "a-2u", A_2u, "PDF/A-2u.";
    A2a => "a-2a", A_2a, "PDF/A-2a.";
    A3b => "a-3b", A_3b, "PDF/A-3b.";
    A3u => "a-3u", A_3u, "PDF/A-3u.";
    A3a => "a-3a", A_3a, "PDF/A-3a.";
    A4 => "a-4", A_4, "PDF/A-4.";
    A4f => "a-4f", A_4f, "PDF/A-4f.";
    A4e => "a-4e", A_4e, "PDF/A-4e.";
    Ua1 => "ua-1", Ua_1, "PDF/UA-1.";
}

impl fmt::Display for PdfStandard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for PdfStandard {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|standard| standard.name() == s)
            .ok_or_else(|| {
                let names: Vec<_> = Self::ALL.iter().map(|standard| standard.name()).collect();
                format!(
                    "unknown PDF standard {s:?}, expected one of: {}",
                    names.join(", ")
                )
            })
    }
}

/// An inclusive range of 1-based page numbers. A missing bound is open:
/// `first: None` starts at the first page, `last: None` runs to the end.
///
/// Parses from `5`, `1-3`, `-3` or `5-`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRange {
    /// First page to export, or `None` for the start of the document.
    pub first: Option<NonZeroUsize>,
    /// Last page to export, or `None` for the end of the document.
    pub last: Option<NonZeroUsize>,
}

impl FromStr for PageRange {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let page = |value: &str| -> Result<Option<NonZeroUsize>, String> {
            if value.is_empty() {
                return Ok(None);
            }
            value
                .parse::<NonZeroUsize>()
                .map(Some)
                .map_err(|_| format!("invalid page number {value:?} in page range {s:?}"))
        };

        let range = match s.split_once('-') {
            Some((first, last)) => Self {
                first: page(first)?,
                last: page(last)?,
            },
            None => {
                let single = page(s)?;
                if single.is_none() {
                    return Err("empty page range".into());
                }
                Self {
                    first: single,
                    last: single,
                }
            }
        };

        if let (Some(first), Some(last)) = (range.first, range.last) {
            if first > last {
                return Err(format!(
                    "page range {s:?} starts after it ends ({first} > {last})"
                ));
            }
        }
        Ok(range)
    }
}
