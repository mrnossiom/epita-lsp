use std::collections::HashMap;

use std::sync::Mutex;
use tower_lsp_server::gen_lsp_types::*;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
	client: Client,
	documents: Mutex<HashMap<Uri, String>>,
}

fn count_lines(start: usize, end: usize, text: &str) -> usize {
	text.lines()
		.skip(start)
		.take(end - start)
		.filter(|line| {
			let t = line.trim();
			!(t.is_empty() || t == "{" || t == "}" || t.starts_with("//"))
		})
		.count()
}

impl LanguageServer for Backend {
	async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
		Ok(InitializeResult {
			capabilities: ServerCapabilities {
				code_action_provider: Some(CodeActionProvider::Bool(true)),
				text_document_sync: Some(TextDocumentSync::Kind(TextDocumentSyncKind::Full)),
				execute_command_provider: Some(ExecuteCommandOptions {
					commands: vec!["epita-cs-lsp-count-lines".to_string()],
					..Default::default()
				}),
				..Default::default()
			},
			..Default::default()
		})
	}

	async fn initialized(&self, _: InitializedParams) {
		self.client
			.log_message(MessageType::Info, "epita-lsp: server initialized!")
			.await;
	}

	async fn shutdown(&self) -> Result<()> {
		Ok(())
	}

	async fn did_open(&self, params: DidOpenTextDocumentParams) {
		let mut owned_documents = self.documents.lock().unwrap();
		owned_documents.insert(params.text_document.uri, params.text_document.text);
	}

	async fn did_change(&self, params: DidChangeTextDocumentParams) {
		let mut owned_documents = self.documents.lock().unwrap();
		let uri = params.text_document.text_document_identifier.uri;
		let content = match params.content_changes.into_iter().last().unwrap() {
			TextDocumentContentChangeEvent::TextDocumentContentChangePartial(c) => c.text,
			TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(c) => c.text,
		};

		owned_documents.insert(uri, content);
	}

	async fn code_action(
		&self,
		params: CodeActionParams,
	) -> Result<Option<Vec<CodeActionResponse>>> {
		let mut actions: Vec<CodeActionResponse> = Vec::new();

		let start = params.range.start;
		let end = params.range.end;
		let owned_documents = self.documents.lock().unwrap();
		let content = owned_documents.get(&params.text_document.uri).unwrap();

		if end.line - start.line >= 10 {
			let lines = count_lines(
				start.line.try_into().unwrap(),
				end.line.try_into().unwrap(),
				content,
			);

			let count_lines_cmd = Command {
				title: "Count lines".to_string(),
				tooltip: Some("Count number of true lines under the cursor".to_string()),
				command: "epita-cs-lsp-count-lines".to_string(),
				arguments: Some(vec![lines.into()]),
			};

			actions.push(CodeActionResponse::Command(count_lines_cmd));
		}

		if actions.is_empty() {
			Ok(None)
		} else {
			Ok(Some(actions))
		}
	}

	async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<LspAny>> {
		if params.command == "epita-cs-lsp-count-lines" {
			let count = &params.arguments.unwrap()[0];
			self.client
				.show_message(MessageType::Info, format!("{count} lines"))
				.await
		}

		Ok(None)
	}
}

#[tokio::main]
async fn main() {
	let stdin = tokio::io::stdin();
	let stdout = tokio::io::stdout();

	let (service, socket) = LspService::new(|client| Backend {
		client,
		documents: Mutex::new(HashMap::new()),
	});
	Server::new(stdin, stdout, socket).serve(service).await;
}
