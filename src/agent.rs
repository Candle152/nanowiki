//! Agent engine — Client enum + ReAct loop + tool dispatch

use crate::config::ProviderConfig;
use crate::output;
use crate::scanner;
use crate::tools;
use rig_core::client::CompletionClient;
use rig_core::completion::message::AssistantContent;
use rig_core::completion::CompletionModel;
use rig_core::completion::request::{CompletionRequest, ToolDefinition};
use rig_core::OneOrMany;
use rig_core::completion::message::Message;
use std::path::Path;
use std::sync::Arc;

pub enum Client {
    OpenAI(rig_core::providers::openai::Client),
    Anthropic(rig_core::providers::anthropic::Client),
    #[cfg(test)]
    Mock(std::cell::RefCell<Vec<AgentResponse>>),
}

impl Client {
    pub fn from_provider_config(pc: &ProviderConfig) -> anyhow::Result<Self> {
        let base_url = pc.base_url.as_deref().unwrap_or(match pc.provider_type {
            crate::config::ProviderType::OpenAI => "https://api.openai.com/v1",
            crate::config::ProviderType::Anthropic => "https://api.anthropic.com",
        });
        match pc.provider_type {
            crate::config::ProviderType::OpenAI => {
                let client = rig_core::providers::openai::Client::builder()
                    .api_key(&pc.api_key)
                    .base_url(base_url)
                    .build()?;
                Ok(Client::OpenAI(client))
            }
            crate::config::ProviderType::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(&pc.api_key)
                    .base_url(base_url)
                    .build()?;
                Ok(Client::Anthropic(client))
            }
        }
    }

    #[cfg(test)]
    pub fn mock(responses: Vec<AgentResponse>) -> Self {
        Client::Mock(std::cell::RefCell::new(responses))
    }

    pub async fn completion(
        &self,
        request: CompletionRequest,
    ) -> anyhow::Result<AgentResponse> {
        #[cfg(test)]
        if let Client::Mock(cell) = self {
            return cell.borrow_mut().pop().ok_or_else(|| anyhow::anyhow!("mock responses exhausted"));
        }

        let model_name = request.model.clone().unwrap_or_default();
        match self {
            #[cfg(test)]
            Client::Mock(_) => unreachable!("Mock already handled above"),
            Client::OpenAI(c) => {
                let model = c.completion_model(&model_name).completions_api();
                retry_completion(|| async {
                    model.completion(request.clone()).await
                        .map_err(|e| e.to_string())
                        .map(|resp| {
                            let mut ar = extract_response(resp.choice);
                            ar.usage = Some(UsageInfo {
                                prompt_tokens: resp.usage.input_tokens as u32,
                                completion_tokens: resp.usage.output_tokens as u32,
                            });
                            ar
                        })
                }).await
            }
            Client::Anthropic(c) => {
                let model = c.completion_model(&model_name);
                retry_completion(|| async {
                    model.completion(request.clone()).await
                        .map_err(|e| e.to_string())
                        .map(|resp| {
                            let mut ar = extract_response(resp.choice);
                            ar.usage = Some(UsageInfo {
                                prompt_tokens: resp.usage.input_tokens as u32,
                                completion_tokens: resp.usage.output_tokens as u32,
                            });
                            ar
                        })
                }).await
            }
        }
    }
}

/// Exponential backoff retry: up to 3 attempts. 4xx errors are not retried.
async fn retry_completion<F, Fut>(mut call: F) -> anyhow::Result<AgentResponse>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<AgentResponse, String>>,
{
    let mut last_err = None;
    for attempt in 0..3u32 {
        match call().await {
            Ok(r) => return Ok(r),
            Err(msg) => {
                // 4xx errors are not retried — they indicate a bad request or auth failure
                if msg.contains("400") || msg.contains("401") || msg.contains("403") {
                    return Err(anyhow::anyhow!("{}", msg));
                }
                last_err = Some(anyhow::anyhow!("{}", msg));
                if attempt < 2 {
                    let wait = 2u64.pow(attempt);
                    eprintln!("LLM call failed (attempt {}), retrying in {}s...", attempt + 1, wait);
                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("unknown error")))
}

pub struct AgentResponse {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCallInfo>,
    /// Non-tool-call content (reasoning/thinking etc.) to pass back to the API
    pub prefix_contents: Vec<AssistantContent>,
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Clone)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

fn extract_response(choice: OneOrMany<AssistantContent>) -> AgentResponse {
    let mut text = String::new();
    let mut tool_calls: Vec<ToolCallInfo> = vec![];
    let mut prefix_contents: Vec<AssistantContent> = vec![];

    for content in choice.iter() {
        match content {
            AssistantContent::Text(t) => {
                text.push_str(&t.text);
                prefix_contents.push(content.clone());
            }
            AssistantContent::ToolCall(tc) => {
                tool_calls.push(ToolCallInfo {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    arguments: tc.function.arguments.clone(),
                });
            }
            // reasoning / thinking / image — must pass back to the API
            other => {
                prefix_contents.push(other.clone());
            }
        }
    }

    let text = if text.is_empty() { None } else { Some(text) };
    AgentResponse {
        text,
        tool_calls,
        prefix_contents,
        usage: None,
    }
}

pub struct ToolDispatcher {
    repo_root: Arc<PathBuf>,
    wiki_root: Arc<PathBuf>,
}

impl ToolDispatcher {
    pub fn new(repo_root: &Path, wiki_root: &Path) -> Self {
        Self {
            repo_root: Arc::from(repo_root.to_path_buf()),
            wiki_root: Arc::from(wiki_root.to_path_buf()),
        }
    }

    pub async fn dispatch(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<String, String> {
        match name {
            "ListDirectory" => {
                let path = args["path"].as_str().unwrap_or("");
                let entries = tools::list_dir::list_directory(&self.repo_root, path)?;
                let lines: Vec<String> = entries
                    .iter()
                    .map(|e| {
                        format!(
                            "{} ({}) - {} bytes",
                            e.name,
                            match e.entry_type {
                                tools::list_dir::EntryType::Dir => "dir",
                                tools::list_dir::EntryType::File => "file",
                            },
                            e.size
                        )
                    })
                    .collect();
                Ok(lines.join("\n"))
            }
            "GlobFiles" => {
                let pattern = args["pattern"].as_str().unwrap_or("*");
                let files = tools::glob::glob_files(&self.repo_root, pattern)?;
                Ok(files.join("\n"))
            }
            "SearchCode" => {
                let pattern = args["pattern"].as_str().unwrap_or("");
                let path = args["path"].as_str();
                let matches =
                    tools::search::search_code(&self.repo_root, pattern, path)?;
                let lines: Vec<String> = matches
                    .iter()
                    .map(|m| format!("{}:{}: {}", m.file_path, m.line_number, m.line_content))
                    .collect();
                Ok(lines.join("\n"))
            }
            "ReadFile" => {
                let file_path = args["file_path"].as_str().unwrap_or("");
                tools::read_file::read_file(&self.repo_root, file_path)
            }
            "WriteWikiPage" => {
                let page_name = args["page_name"].as_str().unwrap_or("");
                let content = args["markdown_content"].as_str().unwrap_or("");
                let summary = args["summary"].as_str().unwrap_or("");
                let result =
                    tools::write_page::write_wiki_page(&self.wiki_root, page_name, content, summary)?;
                if let Err(e) = output::update_index(&self.wiki_root, page_name, summary) {
                    eprintln!("warning: INDEX.md update failed: {}", e);
                }
                Ok(format!(
                    "page {}: {} ({})",
                    if result.created { "created" } else { "updated" },
                    page_name,
                    summary
                ))
            }
            "EditWikiPage" => {
                let page_name = args["page_name"].as_str().unwrap_or("");
                let old_text = args["old_text"].as_str().unwrap_or("");
                let new_text = args["new_text"].as_str().unwrap_or("");
                tools::edit_page::edit_wiki_page(&self.wiki_root, page_name, old_text, new_text)?;
                Ok(format!("page edited: {}", page_name))
            }
            _ => Err(format!("unknown tool: {}", name)),
        }
    }
}

fn tool_icon(name: &str) -> &str {
    match name {
        "ListDirectory" => "📂",
        "ReadFile" => "📖",
        "GlobFiles" => "🔍",
        "SearchCode" => "🔎",
        "WriteWikiPage" => "✍️",
        "EditWikiPage" => "📝",
        _ => "🔧",
    }
}

/// Extract a one-line summary from a tool call's arguments
fn tool_detail(tc: &ToolCallInfo) -> String {
    match tc.name.as_str() {
        "ListDirectory" => {
            let p = tc.arguments["path"].as_str().unwrap_or("");
            if p.is_empty() { "/".into() } else { p.to_string() }
        }
        "ReadFile" => {
            let fp = tc.arguments["file_path"].as_str().unwrap_or("?");
            std::path::Path::new(fp)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(fp)
                .to_string()
        }
        "GlobFiles" => {
            tc.arguments["pattern"].as_str().unwrap_or("*").to_string()
        }
        "SearchCode" => {
            let pat = tc.arguments["pattern"].as_str().unwrap_or("?");
            match tc.arguments["path"].as_str() {
                Some(p) if !p.is_empty() => format!("\"{}\" in {}", pat, p),
                _ => format!("\"{}\"", pat),
            }
        }
        "WriteWikiPage" => {
            tc.arguments["page_name"].as_str().unwrap_or("?").to_string()
        }
        "EditWikiPage" => {
            tc.arguments["page_name"].as_str().unwrap_or("?").to_string()
        }
        _ => "?".into(),
    }
}

/// Format one round of tool calls: same-type calls merged, different types separated by ·
fn format_round(tokens: u64, calls: &[ToolCallInfo]) -> String {
    let mut groups: Vec<(&str, Vec<&ToolCallInfo>)> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for tc in calls {
        if seen.insert(&tc.name) {
            groups.push((&tc.name, calls.iter().filter(|c| c.name == tc.name).collect()));
        }
    }

    let parts: Vec<String> = groups
        .iter()
        .map(|(name, items)| {
            let icon = tool_icon(name);
            let details: Vec<String> = items.iter().map(|tc| tool_detail(tc)).collect();
            format!("{} {}", icon, details.join("  "))
        })
        .collect();

    let token_str = format_tokens(tokens);
    format!("  ◌ ({token_str})  {}", parts.join("  ·  "))
}

fn format_tokens(n: u64) -> String {
    if n < 1_000 {
        format!("{} tokens", n)
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    }
}

pub struct AgentRunner {
    client: Client,
    dispatcher: ToolDispatcher,
    model: String,
}

impl AgentRunner {
    pub fn new(
        client: Client,
        repo_root: &Path,
        wiki_root: &Path,
        model: String,
    ) -> Self {
        Self {
            client,
            dispatcher: ToolDispatcher::new(repo_root, wiki_root),
            model,
        }
    }

    pub async fn run(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<String> {
        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(user_prompt),
        ];

        let mut total_prompt = 0u64;
        let mut total_completion = 0u64;

        loop {
            let request = CompletionRequest {
                model: Some(self.model.clone()),
                chat_history: OneOrMany::many(messages.clone())
                    .expect("chat_history must not be empty"),
                preamble: None,
                tools: tools.clone(),
                tool_choice: None,
                max_tokens: Some(4096),
                temperature: Some(0.3),
                additional_params: None,
                documents: vec![],
                output_schema: None,
                record_telemetry_content: false,
            };

            let response = self.client.completion(request).await?;

            if let Some(ref u) = response.usage {
                total_prompt += u.prompt_tokens as u64;
                total_completion += u.completion_tokens as u64;
            }

            let tool_calls = response.tool_calls;
            if tool_calls.is_empty() {
                let text = response.text.unwrap_or_default();
                let total = total_prompt + total_completion;
                println!("  ✓ done · {} tokens", format_tokens(total));
                return Ok(text);
            }

            println!("{}", format_round(total_prompt + total_completion, &tool_calls));

            for tc in &tool_calls {
                let mut contents: Vec<AssistantContent> = response.prefix_contents.clone();
                contents.push(AssistantContent::ToolCall(
                    rig_core::completion::message::ToolCall::new(
                        tc.id.clone(),
                        rig_core::completion::message::ToolFunction {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    ),
                ));

                let content = if contents.len() == 1 {
                    OneOrMany::one(contents.remove(0))
                } else {
                    OneOrMany::many(contents).expect("contents must not be empty")
                };

                messages.push(Message::Assistant {
                    id: None,
                    content,
                });

                let result = self
                    .dispatcher
                    .dispatch(&tc.name, tc.arguments.clone())
                    .await
                    .unwrap_or_else(|e| format!("error: {}", e));
                messages.push(Message::tool_result(&tc.id, &result));
            }
        }

    }
}

use std::path::PathBuf;

pub const SYSTEM_PROMPT_INIT: &str = include_str!("prompts/system-init.txt");
pub const SYSTEM_PROMPT_UPDATE: &str = include_str!("prompts/system-update.txt");

pub async fn run_init(
    client: Client,
    repo_root: &Path,
    model: &str,
) -> anyhow::Result<()> {
    let wiki_root = repo_root.join(".nanowiki");
    std::fs::create_dir_all(&wiki_root)?;

    let interrupted = output::read_last_update(&wiki_root)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .map(|lu| lu.status == "interrupted")
        .unwrap_or(false);
    if interrupted {
        println!("Detected interrupted run, resuming...");
    }

    println!("Scanning repository files...");
    let files = scanner::scan_repo(repo_root)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let file_list: Vec<String> = files.iter().map(|f| f.relative_path.clone()).collect();
    println!("Found {} files", file_list.len());

    let wiki_goal = if wiki_root.join("INSTRUCTIONS.md").exists() {
        std::fs::read_to_string(wiki_root.join("INSTRUCTIONS.md"))?
    } else {
        "Generate structured documentation for this code repository".to_string()
    };

    let user_prompt = format!(
        "Repository root: {}\n\nFile list ({} files):\n{}\n\n{}",
        repo_root.display(),
        file_list.len(),
        file_list.join("\n"),
        wiki_goal
    );

    let tools = make_tool_definitions();
    let runner = AgentRunner::new(client, repo_root, &wiki_root, model.to_string());
    println!("Analyzing repository...\n");
    let result = runner.run(SYSTEM_PROMPT_INIT, &user_prompt, tools).await;

    match result {
        Ok(text) => {
            println!("\n{}\n", text);
            finalize_init(repo_root, &wiki_root, model, "complete")?;
        }
        Err(e) => {
            eprintln!("Agent error: {}", e);
            finalize_init(repo_root, &wiki_root, model, "interrupted")?;
        }
    }

    Ok(())
}

pub async fn run_update(
    client: Client,
    repo_root: &Path,
    model: &str,
) -> anyhow::Result<()> {
    let wiki_root = repo_root.join(".nanowiki");

    if !wiki_root.exists() {
        anyhow::bail!(".nanowiki/ directory not found, run `nanowiki init` first");
    }

    let last_update = output::read_last_update(&wiki_root)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let git_head = crate::git::get_head_commit(repo_root)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let diff_summary = if let Some(ref lu) = last_update {
        if lu.git_head == git_head {
            println!("No changes detected, nothing to update.");
            return Ok(());
        }
        let changes = crate::git::get_changed_files(repo_root, &lu.git_head)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        format!(
            "Previous commit: {}\nCurrent commit: {}\nChanged files ({}):\n{}",
            lu.git_head,
            git_head,
            changes.len(),
            changes.iter().map(|c| format!("  [{}] {}",
                match c.status {
                    crate::git::ChangeStatus::Added => "+",
                    crate::git::ChangeStatus::Modified => "~",
                    crate::git::ChangeStatus::Deleted => "-",
                }, c.path)).collect::<Vec<_>>().join("\n")
        )
    } else {
        "First incremental update (no previous record), please review all documents.".to_string()
    };

    let user_prompt = format!(
        "Repository: {}\n\nGit changes:\n{}\n\nReview existing .nanowiki/ documents and update only affected pages.",
        repo_root.display(), diff_summary
    );

    let tools = make_tool_definitions();
    let runner = AgentRunner::new(client, repo_root, &wiki_root, model.to_string());
    println!("Analyzing changes...\n");
    let result = runner.run(SYSTEM_PROMPT_UPDATE, &user_prompt, tools).await;

    match result {
        Ok(text) => {
            println!("\n{}\n", text);
            let _ = std::fs::remove_file(wiki_root.join("_plan.md"));
            output::write_last_update(&wiki_root, "update", &git_head, model, "complete")
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            refresh_agents_md(repo_root)?;
            println!("\n✅ NanoWiki update complete!");
        }
        Err(e) => {
            eprintln!("Agent error: {}", e);
            output::write_last_update(&wiki_root, "update", &git_head, model, "interrupted")
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
    }

    Ok(())
}

fn finalize_init(
    repo_root: &Path,
    wiki_root: &Path,
    model: &str,
    status: &str,
) -> anyhow::Result<()> {
    let _ = std::fs::remove_file(wiki_root.join("_skeleton.md"));

    let git_head = crate::git::get_head_commit(repo_root)
        .unwrap_or_else(|_| "unknown".to_string());
    output::write_last_update(wiki_root, "init", &git_head, model, status)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    refresh_agents_md(repo_root)?;

    if status == "complete" {
        println!("\n✅ NanoWiki init complete! Docs generated in .nanowiki/");
    } else {
        println!("\n⚠️  NanoWiki init interrupted. Run again to continue.");
    }

    Ok(())
}

fn refresh_agents_md(repo_root: &Path) -> anyhow::Result<()> {
    let path = repo_root.join("AGENTS.md");
    let nano_block = "## NanoWiki\n\n\
         This repository has documentation located in the /.nanowiki directory.\n\n\
         Start here:\n\n\
         - [NanoWiki quickstart](.nanowiki/quickstart.md)\n\n\
         NanoWiki includes repository overview, architecture notes, workflows, \
         domain concepts, operations, integrations, testing guidance, and source maps.\n\n\
         When working in this repository, read the NanoWiki quickstart first, \
         then follow its links to the relevant architecture, workflow, domain, \
         operation, and testing notes.\n".to_string();

    let start_marker = "<!-- NANOWIKI:START -->";
    let end_marker = "<!-- NANOWIKI:END -->";

    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if content.contains(start_marker) {
            let before = &content[..content.find(start_marker).unwrap() + start_marker.len()];
            let after_start = &content[before.len()..];
            let after = after_start
                .find(end_marker)
                .map(|p| &after_start[p..])
                .unwrap_or("");
            let new_content = format!("{}\n{}\n{}", before, nano_block.trim(), after);
            std::fs::write(&path, new_content)?;
        } else {
            let new_content = format!(
                "{}\n\n{}\n{}\n{}\n",
                content, start_marker, nano_block.trim(), end_marker
            );
            std::fs::write(&path, new_content)?;
        }
    } else {
        let new_content = format!(
            "{}\n{}\n{}\n",
            start_marker, nano_block.trim(), end_marker
        );
        std::fs::write(&path, new_content)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn mock_client(responses: Vec<AgentResponse>) -> Client {
        Client::mock(responses)
    }

    fn tool_call(id: &str, name: &str, args: serde_json::Value) -> ToolCallInfo {
        ToolCallInfo { id: id.into(), name: name.into(), arguments: args }
    }

    fn respond_text(text: &str) -> AgentResponse {
        AgentResponse { text: Some(text.into()), tool_calls: vec![], prefix_contents: vec![], usage: None }
    }

    fn respond_tool_calls(calls: Vec<ToolCallInfo>) -> AgentResponse {
        AgentResponse { text: None, tool_calls: calls, prefix_contents: vec![], usage: None }
    }

    fn setup_dirs() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().to_path_buf();
        let wiki = repo.join(".nanowiki");
        fs::create_dir_all(&wiki).unwrap();
        (tmp, repo, wiki)
    }

    #[tokio::test]
    async fn dispatcher_lists_root_directory() {
        let (_tmp, repo, wiki) = setup_dirs();
        fs::write(repo.join("README.md"), "# Hello").unwrap();
        fs::write(repo.join("Cargo.toml"), "[package]").unwrap();

        let d = ToolDispatcher::new(&repo, &wiki);
        let r = d.dispatch("ListDirectory", serde_json::json!({"path": ""})).await.unwrap();
        assert!(r.contains("README.md"));
        assert!(r.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn dispatcher_reads_file() {
        let (_tmp, repo, wiki) = setup_dirs();
        fs::write(repo.join("main.rs"), "fn main() {}").unwrap();

        let d = ToolDispatcher::new(&repo, &wiki);
        let r = d.dispatch("ReadFile", serde_json::json!({"file_path": "main.rs"})).await.unwrap();
        assert_eq!(r, "fn main() {}");
    }

    #[tokio::test]
    async fn dispatcher_writes_wiki_page() {
        let (_tmp, repo, wiki) = setup_dirs();

        let d = ToolDispatcher::new(&repo, &wiki);
        let r = d.dispatch("WriteWikiPage", serde_json::json!({
            "page_name": "arch.md",
            "markdown_content": "# Arch",
            "summary": "architecture"
        })).await.unwrap();
        assert!(r.contains("arch.md"));
        assert!(r.contains("created"));
        assert!(wiki.join("arch.md").exists());
        assert!(wiki.join("INDEX.md").exists());
    }

    #[tokio::test]
    async fn dispatcher_edits_wiki_page() {
        let (_tmp, repo, wiki) = setup_dirs();
        fs::write(wiki.join("arch.md"), "## Old Title\ncontent").unwrap();

        let d = ToolDispatcher::new(&repo, &wiki);
        let r = d.dispatch("EditWikiPage", serde_json::json!({
            "page_name": "arch.md",
            "old_text": "## Old Title",
            "new_text": "## New Title"
        })).await.unwrap();
        assert!(r.contains("edited"));
        let c = fs::read_to_string(wiki.join("arch.md")).unwrap();
        assert!(c.contains("New Title"));
    }

    #[tokio::test]
    async fn dispatcher_rejects_unknown_tool() {
        let (_tmp, repo, wiki) = setup_dirs();
        let d = ToolDispatcher::new(&repo, &wiki);
        let r = d.dispatch("NonExistent", serde_json::json!({})).await;
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("unknown tool"));
    }

    #[tokio::test]
    async fn dispatcher_searches_code() {
        let (_tmp, repo, wiki) = setup_dirs();
        fs::write(repo.join("lib.rs"), "pub fn hello() -> &'static str {\n    \"hello\"\n}").unwrap();

        let d = ToolDispatcher::new(&repo, &wiki);
        let r = d.dispatch("SearchCode", serde_json::json!({"pattern": "hello"})).await.unwrap();
        assert!(r.contains("lib.rs"));
    }

    #[tokio::test]
    async fn dispatcher_globs_files() {
        let (_tmp, repo, wiki) = setup_dirs();
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/main.rs"), "").unwrap();
        fs::write(repo.join("src/lib.rs"), "").unwrap();

        let d = ToolDispatcher::new(&repo, &wiki);
        let r = d.dispatch("GlobFiles", serde_json::json!({"pattern": "src/**/*.rs"})).await.unwrap();
        assert!(r.contains("src/main.rs"));
        assert!(r.contains("src/lib.rs"));
    }

    #[tokio::test]
    async fn agent_stops_when_no_tool_calls() {
        let (_tmp, repo, wiki) = setup_dirs();
        let client = mock_client(vec![respond_text("task complete")]);
        let runner = AgentRunner::new(client, &repo, &wiki, "mock".into());

        let result = runner.run("system", "user", make_tool_definitions()).await.unwrap();
        assert_eq!(result, "task complete");
    }

    #[tokio::test]
    async fn agent_executes_tool_and_continues() {
        let (_tmp, repo, wiki) = setup_dirs();
        fs::write(repo.join("README.md"), "# Hello World").unwrap();

        let client = mock_client(vec![
            respond_tool_calls(vec![
                tool_call("c1", "ReadFile", serde_json::json!({"file_path": "README.md"}))
            ]),
            respond_text("analysis complete"),
        ]);
        let runner = AgentRunner::new(client, &repo, &wiki, "mock".into());

        let result = runner.run("sys", "user", make_tool_definitions()).await.unwrap();
        assert_eq!(result, "analysis complete");
    }

    #[tokio::test]
    async fn agent_writes_and_reads_wiki_page() {
        let (_tmp, repo, wiki) = setup_dirs();

        let client = mock_client(vec![
            respond_tool_calls(vec![
                tool_call("c1", "WriteWikiPage", serde_json::json!({
                    "page_name": "overview.md",
                    "markdown_content": "# Overview\ncontent",
                    "summary": "overview"
                }))
            ]),
            respond_tool_calls(vec![
                tool_call("c2", "ReadFile", serde_json::json!({"file_path": ".nanowiki/overview.md"}))
            ]),
            respond_text("documentation generated"),
        ]);
        let runner = AgentRunner::new(client, &repo, &wiki, "mock".into());

        let result = runner.run("sys", "user", make_tool_definitions()).await.unwrap();
        assert_eq!(result, "documentation generated");
    }

    #[test]
    fn extract_text_response() {
        let text = AssistantContent::Text(rig_core::completion::message::Text {
            text: "hello".into(),
            additional_params: None,
        });
        let resp = extract_response(OneOrMany::one(text));
        assert_eq!(resp.text.unwrap(), "hello");
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn extract_tool_call_response() {
        let tc = AssistantContent::ToolCall(rig_core::completion::message::ToolCall::new(
            "id1".into(),
            rig_core::completion::message::ToolFunction {
                name: "ReadFile".into(),
                arguments: serde_json::json!({"file_path": "main.rs"}),
            },
        ));
        let resp = extract_response(OneOrMany::one(tc));
        assert!(resp.text.is_none());
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "ReadFile");
    }

    #[test]
    fn refresh_creates_new_agents_md() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        refresh_agents_md(root).unwrap();
        let content = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(content.contains("NANOWIKI:START"));
        assert!(content.contains("NANOWIKI:END"));
        assert!(content.contains("NanoWiki quickstart"));
    }

    #[test]
    fn refresh_preserves_content_outside_markers() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let existing = "# My Project\n\n<!-- NANOWIKI:START -->\nold block\n<!-- NANOWIKI:END -->\n\nMore stuff";
        fs::write(root.join("AGENTS.md"), existing).unwrap();

        refresh_agents_md(root).unwrap();
        let content = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(content.starts_with("# My Project"));
        assert!(content.contains("More stuff"));
        assert!(content.contains("NanoWiki quickstart"));
        assert!(!content.contains("old block"));
    }

    #[test]
    fn refresh_appends_markers_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("AGENTS.md"), "# Just a readme").unwrap();

        refresh_agents_md(root).unwrap();
        let content = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(content.starts_with("# Just a readme"));
        assert!(content.contains("NANOWIKI:START"));
    }
}

fn make_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "ListDirectory".into(),
            description: "List directory contents (files and subdirectories)".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to repository root. Empty string or . for root."}
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "GlobFiles".into(),
            description: "Match file paths by glob pattern. Max 500 entries.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern, e.g. src/**/*.rs"}
                },
                "required": ["pattern"]
            }),
        },
        ToolDefinition {
            name: "SearchCode".into(),
            description: "Search files for a text pattern (plain substring, case-sensitive). Max 100 matches.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Search pattern"},
                    "path": {"type": "string", "description": "Optional: limit search scope"}
                },
                "required": ["pattern"]
            }),
        },
        ToolDefinition {
            name: "ReadFile".into(),
            description: "Read a single file from the repository. UTF-8 text only, truncated at 50KB. Sensitive files rejected.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File path relative to repository root"}
                },
                "required": ["file_path"]
            }),
        },
        ToolDefinition {
            name: "WriteWikiPage".into(),
            description: "Create or overwrite a Markdown document under .nanowiki/.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "page_name": {"type": "string", "description": "Document path (e.g. architecture/overview.md)"},
                    "markdown_content": {"type": "string", "description": "Markdown content"},
                    "summary": {"type": "string", "description": "One-line summary (20-100 chars)"}
                },
                "required": ["page_name", "markdown_content", "summary"]
            }),
        },
        ToolDefinition {
            name: "EditWikiPage".into(),
            description: "Precisely replace text in a .nanowiki/ document.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "page_name": {"type": "string", "description": "Document path"},
                    "old_text": {"type": "string", "description": "Text to replace (must match uniquely)"},
                    "new_text": {"type": "string", "description": "Replacement text"}
                },
                "required": ["page_name", "old_text", "new_text"]
            }),
        },
    ]
}
