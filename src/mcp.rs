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
creation_timestamp so the output is reproducible. `list_templates` shows the templates that \
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
        let options = options(&params.document, standards);
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
