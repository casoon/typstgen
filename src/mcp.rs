//! `typstgen mcp`: a Model Context Protocol server over stdio.
//!
//! Lets an AI agent check, preview and compile `.typ` files in a workspace
//! directory. Every path a tool receives is relative to that directory, and
//! no tool reads or writes outside it (imports from template directories and
//! packages are resolved as usual).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use serde::Deserialize;

use crate::{CompileOptions, Config, PdfStandard};

const INSTRUCTIONS: &str = "\
Compile Typst documents (.typ) to PDF with an embedded Typst engine. Paths are relative to the \
workspace directory. Typical loop: write or edit a .typ file, call `check` and fix every reported \
error (each line names file:line:column), look at a page with `preview`, then `compile`. For \
archiving, compile with pdf_standards [\"a-2b\"] (or \"a-3b\" with attachments) and a \
creation_timestamp so the output is reproducible. For a ZUGFeRD e-invoice pass zugferd {invoice, \
company} (business-templates JSON files) with pdf_standards [\"a-3b\"]; problems in the data are listed per \
field. `e_invoice` writes the same data as XRechnung or Peppol BIS Billing XML (UBL), without sending it. `prepare_invoice` reads user-owned tax and presentation configuration. \
`list_document_profiles`, `resolve_document_profile` and `validate_document` use a package catalog \
for explicit profile selection and business checks; external validation and transmission stay separate. \
`list_templates` shows the templates that \
#import can use. Typst packages (#import \"@preview/name:version\") are resolved automatically.";

/// Values shared by `check`, `preview` and `compile`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DocumentParams {
    /// Path of the .typ file, relative to the workspace.
    path: String,
    /// String values for `sys.inputs`, e.g. {"customer": "ACME"}.
    #[serde(default)]
    inputs: BTreeMap<String, String>,
    /// Unix timestamp used as creation date and for datetime.today().
    creation_timestamp: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CompileParams {
    #[serde(flatten)]
    document: DocumentParams,
    /// Output PDF path relative to the workspace. Default: the input path with .pdf.
    output: Option<String>,
    /// PDF standards to enforce, e.g. ["a-2b"], ["a-3b"], ["ua-1"], ["a-2a", "ua-1"].
    #[serde(default)]
    pdf_standards: Vec<String>,
    /// Embed a ZUGFeRD / Factur-X e-invoice (EN 16931) built from business-templates JSON
    /// files in the workspace. Needs pdf_standards ["a-3b"].
    #[cfg(feature = "zugferd")]
    zugferd: Option<ZugferdParams>,
}

#[cfg(feature = "zugferd")]
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ZugferdParams {
    /// Invoice data (business-templates invoice JSON), relative to the workspace.
    invoice: String,
    /// Seller data (company.json), relative to the workspace.
    company: String,
}

#[cfg(feature = "zugferd")]
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EInvoiceParams {
    #[serde(flatten)]
    data: ZugferdParams,
    /// Target format: "xrechnung-3.0" or "peppol-bis-3.0" (UBL 2.1).
    format: String,
    /// Output XML path relative to the workspace. Default: the invoice path with .xml.
    output: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PreviewParams {
    #[serde(flatten)]
    document: DocumentParams,
    /// 1-based page number. Default 1.
    page: Option<usize>,
    /// Pixels per point: 1.0 = 72 dpi. Default 1.5. Maximum 4.
    scale: Option<f64>,
}

#[cfg(feature = "zugferd")]
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ProfileParams {
    /// Package catalog JSON path, relative to the workspace.
    catalog: String,
    /// Include draft and unsupported entries in the capability list.
    #[serde(default)]
    include_drafts: bool,
    /// Explicit profile identity and separate rendering/output choices.
    #[serde(default)]
    selection: serde_json::Value,
    /// Invoice validation envelope: selection, data, company and explicit facts.
    #[serde(default)]
    request: serde_json::Value,
}

#[cfg(feature = "zugferd")]
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct InvoiceConfigParams {
    /// Invoice configuration JSON path, relative to the workspace.
    config: String,
    /// Invoice envelope with company, data and optional facts.
    request: serde_json::Value,
}

#[derive(Clone)]
pub struct TypstgenServer {
    root: Arc<PathBuf>,
    config: Arc<Config>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl TypstgenServer {
    /// A server for the workspace `root` (canonicalized) with `config`.
    pub fn new(root: PathBuf, config: Config) -> Self {
        Self {
            root: Arc::new(root),
            config: Arc::new(config),
            tool_router: Self::tool_router(),
        }
    }

    #[cfg(feature = "zugferd")]
    fn profile_catalog(&self, path: &str) -> Result<serde_json::Value, String> {
        let input = self.input(path)?;
        let bytes = std::fs::read(input).map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }

    #[cfg(feature = "zugferd")]
    #[tool(
        description = "List versioned document profiles and separate country, tax case, language, layout, output and transmission capabilities. Draft profiles are opt-in."
    )]
    async fn list_document_profiles(
        &self,
        Parameters(params): Parameters<ProfileParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(match self.profile_catalog(&params.catalog) {
            Ok(catalog) => success(
                crate::profiles::list_document_profiles(&catalog, params.include_drafts)
                    .to_string(),
            ),
            Err(error) => failure(error),
        })
    }

    #[cfg(feature = "zugferd")]
    #[tool(
        description = "Resolve an explicit versioned document profile. Returns unsupported choices and missing selection fields; language does not change country rules."
    )]
    async fn resolve_document_profile(
        &self,
        Parameters(params): Parameters<ProfileParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(match self.profile_catalog(&params.catalog) {
            Ok(catalog) => success(
                crate::profiles::resolve_document_profile(&catalog, &params.selection).to_string(),
            ),
            Err(error) => failure(error),
        })
    }

    #[cfg(feature = "zugferd")]
    #[tool(
        description = "Validate invoice arithmetic and package profile rules against explicit facts. Returns field paths, rule IDs, sources and normalized invoice data. External EN16931/PDF-A validation and transmission remain separate not_run statuses. Does not invent facts."
    )]
    async fn validate_document(
        &self,
        Parameters(params): Parameters<ProfileParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(match self.profile_catalog(&params.catalog) {
            Ok(catalog) => {
                success(crate::profiles::validate_document(&catalog, &params.request).to_string())
            }
            Err(error) => failure(error),
        })
    }

    #[cfg(feature = "zugferd")]
    #[tool(
        description = "Prepare invoice amounts, tax groups and rendering data using a user-owned configuration file. Selects explicit tax keys without inferring national tax law. VAT invoices can supply CII; sales-tax invoices return additive component data for PDF. Jurisdiction is not assessed."
    )]
    async fn prepare_invoice(
        &self,
        Parameters(params): Parameters<InvoiceConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(match self.profile_catalog(&params.config) {
            Ok(config) => success(
                crate::invoice_config::prepare_invoice(&config, &params.request).to_string(),
            ),
            Err(error) => failure(error),
        })
    }

    #[cfg(feature = "zugferd")]
    #[tool(
        description = "Write a pure-XML e-invoice (XRechnung 3.0 or Peppol BIS Billing 3.0, UBL 2.1) from business-templates invoice and company JSON. Problems are listed per JSON field. Returns the check states: schema, EN16931 and profile Schematron validation and transmission stay not_run. Does not send the invoice."
    )]
    async fn e_invoice(
        &self,
        Parameters(params): Parameters<EInvoiceParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = (|| {
            let format = params
                .format
                .parse::<crate::zugferd::UblFormat>()
                .map_err(|error| error.to_string())?;
            let invoice = self.input(&params.data.invoice)?;
            let company = self.input(&params.data.company)?;
            let output = match params.output.as_deref() {
                Some(path) => self.output(path)?,
                None => invoice.with_extension("xml"),
            };
            let read = |path: &Path| {
                std::fs::read_to_string(path)
                    .map_err(|error| format!("{}: {error}", path.display()))
            };
            let xml =
                crate::zugferd::ubl_xml_from_docgen(format, &read(&invoice)?, &read(&company)?)
                    .map_err(|error| error.to_string())?;
            std::fs::write(&output, xml)
                .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
            let checks: serde_json::Map<_, _> = format
                .checks()
                .iter()
                .map(|(check, state)| ((*check).to_owned(), serde_json::json!(state)))
                .collect();
            Ok::<_, String>(serde_json::json!({
                "output": self.relative(&output),
                "format": format.id(),
                "syntax": "UBL 2.1",
                "customization_id": format.customization_id(),
                "profile_id": crate::zugferd::PROFILE_ID,
                "checks": checks,
            }))
        })();
        Ok(match result {
            Ok(report) => success(report.to_string()),
            Err(error) => failure(error),
        })
    }

    #[tool(
        description = "Compile a .typ file without writing anything. Returns the page count and \
                       warnings, or every error as path:line:column with hints."
    )]
    async fn check(
        &self,
        Parameters(params): Parameters<DocumentParams>,
    ) -> Result<CallToolResult, McpError> {
        let input = match self.input(&params.path) {
            Ok(input) => input,
            Err(message) => return Ok(failure(message)),
        };
        let options = options(&params, Vec::new());
        let config = Arc::clone(&self.config);
        let result =
            blocking(move || crate::compile_with_options(&input, &config, &options)).await?;
        Ok(match result {
            Ok(output) => success(summary("ok", output.pages, &output.warnings)),
            Err(error) => failure(error.to_string()),
        })
    }

    #[tool(description = "Render one page of a .typ file as a PNG image to look at the layout.")]
    async fn preview(
        &self,
        Parameters(params): Parameters<PreviewParams>,
    ) -> Result<CallToolResult, McpError> {
        let input = match self.input(&params.document.path) {
            Ok(input) => input,
            Err(message) => return Ok(failure(message)),
        };
        let page = params.page.unwrap_or(1);
        let scale = params.scale.unwrap_or(1.5).clamp(0.25, 4.0);
        let options = options(&params.document, Vec::new());
        let config = Arc::clone(&self.config);
        let result =
            blocking(move || crate::render_png(&input, &config, &options, page, scale)).await?;
        Ok(match result {
            Ok(png) => CallToolResult::success(vec![ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(png),
                "image/png",
            )]),
            Err(error) => failure(error.to_string()),
        })
    }

    #[tool(
        description = "Compile a .typ file to PDF and write it into the workspace. Optionally \
                       enforce PDF standards such as PDF/A-2b or PDF/UA-1."
    )]
    async fn compile(
        &self,
        Parameters(params): Parameters<CompileParams>,
    ) -> Result<CallToolResult, McpError> {
        let input = match self.input(&params.document.path) {
            Ok(input) => input,
            Err(message) => return Ok(failure(message)),
        };
        let output = match params.output.as_deref() {
            Some(path) => self.output(path),
            None => Ok(input.with_extension("pdf")),
        };
        let output = match output {
            Ok(output) => output,
            Err(message) => return Ok(failure(message)),
        };
        let standards: Result<Vec<PdfStandard>, String> = params
            .pdf_standards
            .iter()
            .map(|name| {
                name.parse::<PdfStandard>()
                    .map_err(|error| error.to_string())
            })
            .collect();
        let standards = match standards {
            Ok(standards) => standards,
            Err(message) => return Ok(failure(message)),
        };
        #[allow(unused_mut)]
        let mut options = options(&params.document, standards);
        #[cfg(feature = "zugferd")]
        if let Some(zugferd) = &params.zugferd {
            let xml = self
                .input(&zugferd.invoice)
                .and_then(|invoice| Ok((invoice, self.input(&zugferd.company)?)))
                .and_then(|(invoice, company)| {
                    let read = |path: &Path| {
                        std::fs::read_to_string(path)
                            .map_err(|error| format!("{}: {error}", path.display()))
                    };
                    crate::zugferd::xml_from_docgen(&read(&invoice)?, &read(&company)?)
                        .map_err(|error| error.to_string())
                });
            match xml {
                Ok(xml) => options.zugferd_xml = Some(xml.into_bytes()),
                Err(message) => return Ok(failure(message)),
            }
        }
        let config = Arc::clone(&self.config);
        let result =
            blocking(move || crate::compile_with_options(&input, &config, &options)).await?;
        let compiled = match result {
            Ok(compiled) => compiled,
            Err(error) => return Ok(failure(error.to_string())),
        };
        if let Err(error) = std::fs::write(&output, &compiled.pdf) {
            return Ok(failure(format!(
                "failed to write {}: {error}",
                output.display()
            )));
        }
        let head = format!(
            "wrote {} ({} bytes)",
            self.relative(&output),
            compiled.pdf.len()
        );
        Ok(success(summary(&head, compiled.pages, &compiled.warnings)))
    }

    #[tool(
        description = "List the .typ templates in the configured template directories, with the \
                       first comment line of each file as its description."
    )]
    async fn list_templates(&self) -> Result<CallToolResult, McpError> {
        let mut lines = Vec::new();
        for root in self.config.template_roots() {
            let mut files = Vec::new();
            collect_typ_files(&root, &mut files);
            files.sort();
            for file in files {
                let path = file
                    .strip_prefix(&root)
                    .unwrap_or(&file)
                    .display()
                    .to_string();
                match first_comment(&file) {
                    Some(description) => lines.push(format!("{path} — {description}")),
                    None => lines.push(path),
                }
            }
        }
        if lines.is_empty() {
            lines.push("no templates found (template_paths in typstgen.toml)".to_owned());
        }
        Ok(success(lines.join("\n")))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TypstgenServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("typstgen", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

impl TypstgenServer {
    /// An existing file inside the workspace.
    fn input(&self, path: &str) -> Result<PathBuf, String> {
        let resolved = self
            .root
            .join(path)
            .canonicalize()
            .map_err(|error| format!("{path}: {error}"))?;
        self.inside(path, resolved)
    }

    /// A file to write inside the workspace; its directory must exist.
    fn output(&self, path: &str) -> Result<PathBuf, String> {
        let joined = self.root.join(path);
        let name = joined
            .file_name()
            .ok_or_else(|| format!("{path}: not a file path"))?
            .to_owned();
        let parent = joined
            .parent()
            .unwrap_or(&self.root)
            .canonicalize()
            .map_err(|error| format!("{path}: {error}"))?;
        self.inside(path, parent.join(name))
    }

    fn inside(&self, path: &str, resolved: PathBuf) -> Result<PathBuf, String> {
        if resolved.starts_with(self.root.as_path()) {
            Ok(resolved)
        } else {
            Err(format!(
                "{path}: outside the workspace {}",
                self.root.display()
            ))
        }
    }

    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(self.root.as_path())
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

fn options(params: &DocumentParams, pdf_standards: Vec<PdfStandard>) -> CompileOptions {
    CompileOptions {
        inputs: params.inputs.clone(),
        pdf_standards,
        creation_timestamp: params.creation_timestamp,
        ..CompileOptions::default()
    }
}

/// Runs the CPU-bound compilation off the async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, McpError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))
}

fn summary(head: &str, pages: usize, warnings: &[String]) -> String {
    let mut text = format!("{head}, {pages} page{}", if pages == 1 { "" } else { "s" });
    for warning in warnings {
        text.push('\n');
        text.push_str(warning);
    }
    text
}

fn success(text: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
}

/// A tool-level error the agent can read and act on.
fn failure(text: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(text)])
}

fn collect_typ_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_typ_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "typ") {
            files.push(path);
        }
    }
}

/// The text of the first `//` comment at the top of a file.
fn first_comment(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let comment = line.strip_prefix("//")?.trim_start_matches('/').trim();
    (!comment.is_empty()).then(|| comment.to_owned())
}

/// Serves MCP on stdin/stdout until the client disconnects.
pub fn serve(root: PathBuf, config: Config) -> std::io::Result<()> {
    use rmcp::ServiceExt;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let service = TypstgenServer::new(root, config)
            .serve(rmcp::transport::stdio())
            .await
            .map_err(std::io::Error::other)?;
        service.waiting().await.map_err(std::io::Error::other)?;
        Ok(())
    })
}
