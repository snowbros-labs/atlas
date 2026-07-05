//! Language Server Protocol server for editor integration.
//!
//! Speaks LSP over stdio. On `didOpen` and `didSave` the whole project
//! is re-analyzed through [`snowbros_engine`] (the incremental cache
//! makes warm runs take tens of milliseconds) and findings are published
//! as diagnostics per file. Files whose findings disappeared get an
//! empty publish so stale squiggles are cleared.
//!
//! The entry point [`run_stdio`] is blocking and owns its own tokio
//! runtime, so callers (the CLI) stay async-free.

use std::collections::HashSet;

use camino::Utf8PathBuf;
use tokio::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc::Result as JsonRpcResult;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, InitializeParams, InitializeResult, InitializedParams,
    MessageType, NumberOrString, Position, Range, SaveOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

use snowbros_core::Severity;

/// Maps a SNOWBROS severity onto the LSP scale.
fn lsp_severity(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Critical | Severity::High => DiagnosticSeverity::ERROR,
        Severity::Medium => DiagnosticSeverity::WARNING,
        Severity::Low => DiagnosticSeverity::INFORMATION,
        Severity::Info => DiagnosticSeverity::HINT,
    }
}

/// Converts a 1-based core position to a 0-based LSP position.
fn lsp_position(p: snowbros_core::Position) -> Position {
    Position {
        line: p.line.saturating_sub(1),
        character: p.column.saturating_sub(1),
    }
}

/// Converts a core diagnostic to an LSP diagnostic.
fn lsp_diagnostic(d: &snowbros_core::Diagnostic) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: lsp_position(d.location.span.start),
            end: lsp_position(d.location.span.end),
        },
        severity: Some(lsp_severity(d.severity)),
        code: Some(NumberOrString::String(d.rule_id.clone())),
        source: Some("snowbros".to_string()),
        message: d.title.clone(),
        ..Diagnostic::default()
    }
}

/// The language server state.
struct Backend {
    client: Client,
    /// Project root, captured during `initialize`.
    root: RwLock<Option<Utf8PathBuf>>,
    /// URIs we last published non-empty diagnostics for, so stale ones
    /// can be cleared on the next run.
    published: Mutex<HashSet<Url>>,
}

impl Backend {
    /// Runs a full (cache-accelerated) analysis and republishes all
    /// diagnostics, clearing files that no longer have findings.
    async fn analyze_and_publish(&self) {
        let Some(root) = self.root.read().await.clone() else {
            return;
        };

        // The engine is synchronous and rayon-parallel — keep it off the
        // async executor threads.
        let analysis = {
            let root = root.clone();
            tokio::task::spawn_blocking(move || snowbros_engine::analyze(&root, true)).await
        };
        let analysis = match analysis {
            Ok(Ok(a)) => a,
            Ok(Err(message)) => {
                self.client
                    .log_message(MessageType::ERROR, format!("analysis failed: {message}"))
                    .await;
                return;
            }
            Err(join_error) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("analysis panicked: {join_error}"),
                    )
                    .await;
                return;
            }
        };

        // Group findings per absolute file URI. BTreeMap-free: order
        // within a file is already deterministic (report is sorted).
        let mut by_uri: Vec<(Url, Vec<Diagnostic>)> = Vec::new();
        for d in &analysis.report.diagnostics {
            let abs = root.join(&d.location.file);
            let Ok(uri) = Url::from_file_path(abs.as_std_path()) else {
                continue;
            };
            match by_uri.iter_mut().find(|(u, _)| *u == uri) {
                Some((_, list)) => list.push(lsp_diagnostic(d)),
                None => by_uri.push((uri, vec![lsp_diagnostic(d)])),
            }
        }

        let mut published = self.published.lock().await;
        let current: HashSet<Url> = by_uri.iter().map(|(u, _)| u.clone()).collect();
        // Clear files that had findings last run but not this run.
        for stale in published.difference(&current) {
            self.client
                .publish_diagnostics(stale.clone(), Vec::new(), None)
                .await;
        }
        for (uri, diagnostics) in by_uri {
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
        *published = current;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> JsonRpcResult<InitializeResult> {
        // Prefer workspace folders; fall back to the deprecated root_uri.
        #[allow(deprecated)]
        let root_uri = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first().map(|f| f.uri.clone()))
            .or(params.root_uri);
        if let Some(uri) = root_uri {
            if let Ok(path) = uri.to_file_path() {
                if let Ok(utf8) = Utf8PathBuf::from_path_buf(path) {
                    *self.root.write().await = Some(utf8);
                }
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::NONE),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(false),
                        })),
                        ..TextDocumentSyncOptions::default()
                    },
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "snowbros".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "SNOWBROS language server ready")
            .await;
        self.analyze_and_publish().await;
    }

    async fn did_open(&self, _: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        self.analyze_and_publish().await;
    }

    async fn did_save(&self, _: tower_lsp::lsp_types::DidSaveTextDocumentParams) {
        self.analyze_and_publish().await;
    }

    async fn shutdown(&self) -> JsonRpcResult<()> {
        Ok(())
    }
}

/// Serves LSP over stdio until the client disconnects. Blocking: builds
/// its own tokio runtime so the caller needs no async machinery.
pub fn run_stdio() -> Result<(), String> {
    let runtime =
        tokio::runtime::Runtime::new().map_err(|e| format!("cannot start runtime: {e}"))?;
    runtime.block_on(async {
        let (service, socket) = LspService::new(|client| Backend {
            client,
            root: RwLock::new(None),
            published: Mutex::new(HashSet::new()),
        });
        Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
            .serve(service)
            .await;
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{Confidence, SourceLocation, Span};

    fn span() -> Span {
        Span::new(
            snowbros_core::Position::new(3, 5),
            snowbros_core::Position::new(3, 9),
            0,
            4,
        )
    }

    #[test]
    fn severity_mapping_covers_all_levels() {
        assert_eq!(lsp_severity(Severity::Critical), DiagnosticSeverity::ERROR);
        assert_eq!(lsp_severity(Severity::High), DiagnosticSeverity::ERROR);
        assert_eq!(lsp_severity(Severity::Medium), DiagnosticSeverity::WARNING);
        assert_eq!(lsp_severity(Severity::Low), DiagnosticSeverity::INFORMATION);
        assert_eq!(lsp_severity(Severity::Info), DiagnosticSeverity::HINT);
    }

    #[test]
    fn positions_convert_one_based_to_zero_based() {
        let p = lsp_position(snowbros_core::Position::new(1, 1));
        assert_eq!((p.line, p.character), (0, 0));
        let p = lsp_position(snowbros_core::Position::new(10, 42));
        assert_eq!((p.line, p.character), (9, 41));
    }

    #[test]
    fn diagnostic_carries_rule_id_and_source() {
        let d = snowbros_core::Diagnostic::new(
            "security/no-eval",
            "eval() call",
            "eval executes arbitrary code",
            "security",
            Severity::High,
            Confidence::Certain,
            SourceLocation::new("src/a.ts", span()),
        );
        let lsp = lsp_diagnostic(&d);
        assert_eq!(
            lsp.code,
            Some(NumberOrString::String("security/no-eval".to_string()))
        );
        assert_eq!(lsp.source.as_deref(), Some("snowbros"));
        assert_eq!(lsp.range.start, Position::new(2, 4));
        assert_eq!(lsp.range.end, Position::new(2, 8));
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::ERROR));
    }
}
