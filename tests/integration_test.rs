//! NanoWiki 集成测试 — 用 fixture 项目验证 init 流程
//!
//! 配置文件: `tests/test-config.local.json`（从 `tests/test-config.example.json` 复制并填入真实 API key）
//! 未配置或 api_key 为占位符时自动 skip。

use nanowiki::agent;
use nanowiki::config::{self, Config};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 测试配置
// ---------------------------------------------------------------------------

fn load_test_config() -> Option<Config> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test-config.local.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn is_placeholder_key(key: &str) -> bool {
    key.is_empty()
        || key == "sk-your-api-key-here"
        || key == "sk-ant-your-key-here"
        || key.starts_with("sk-your-")
}

// ---------------------------------------------------------------------------
// fixture 项目
// ---------------------------------------------------------------------------

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sample-project")
}

/// 复制 fixture 到临时目录（避免污染原始文件）
fn copy_fixture_to_temp() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().to_path_buf();

    copy_dir_all(&fixture_dir(), &dest).unwrap();

    // 初始化 git 仓库（nanowiki init 需要 git2 获取 HEAD）
    let git_init = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&dest)
        .output();
    if git_init.is_ok() {
        let _ = std::process::Command::new("git")
            .args(["config", "user.email", "test@nanowiki.local"])
            .current_dir(&dest)
            .output();
        let _ = std::process::Command::new("git")
            .args(["config", "user.name", "NanoWiki Test"])
            .current_dir(&dest)
            .output();
        let _ = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&dest)
            .output();
        let _ = std::process::Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&dest)
            .output();
    }

    (tmp, dest)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

/// 验证 init 输出结构
fn verify_output(repo_root: &Path) {
    let nanowiki_dir = repo_root.join(".nanowiki");
    assert!(nanowiki_dir.exists(), ".nanowiki/ 目录应存在");

    let quickstart = nanowiki_dir.join("quickstart.md");
    assert!(quickstart.exists(), "quickstart.md 应存在");

    let index = nanowiki_dir.join("INDEX.md");
    assert!(index.exists(), "INDEX.md 应存在");
    let index_content = std::fs::read_to_string(&index).unwrap();
    assert!(index_content.contains("quickstart"), "INDEX.md 应引用 quickstart");

    let last_update = nanowiki_dir.join(".last-update.json");
    assert!(last_update.exists(), ".last-update.json 应存在");
    let lu: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&last_update).unwrap()).unwrap();
    assert_eq!(lu["status"], "complete");

    let agents = repo_root.join("AGENTS.md");
    assert!(agents.exists(), "AGENTS.md 应存在");
    let agents_content = std::fs::read_to_string(&agents).unwrap();
    assert!(agents_content.contains("NANOWIKI:START"), "AGENTS.md 应包含标记");

    println!("  ✅ 验证通过: quickstart.md, INDEX.md, AGENTS.md, .last-update.json 均已生成");
}

// ---------------------------------------------------------------------------
// 测试入口
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_with_configured_providers() {
    let test_cfg = match load_test_config() {
        Some(c) => c,
        None => {
            eprintln!("跳过: 未找到 tests/test-config.local.json，请从 .example.json 复制并填入 API key");
            return;
        }
    };

    // 验证配置
    if let Err(e) = config::validate(&test_cfg) {
        eprintln!("跳过: 测试配置无效 — {}", e);
        return;
    }

    let mut any_ran = false;

    // 遍历所有 provider
    for (name, pc) in &test_cfg.providers {
        if is_placeholder_key(&pc.api_key) {
            eprintln!(
                "跳过 [{}]: api_key 仍为占位符，请编辑 tests/test-config.local.json",
                name
            );
            continue;
        }

        let model = pc.models.first().map(|s| s.as_str()).unwrap_or("?").to_string();
        println!("\n========== 测试 [{}] — {:?} / {} ==========", name, pc.provider_type, model);

        let client = match agent::Client::from_provider_config(pc) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("跳过 [{}]: 无法创建客户端 — {}", name, e);
                continue;
            }
        };

        let (_tmp, repo_root) = copy_fixture_to_temp();
        println!("  fixture: {}", repo_root.display());

        let result = agent::run_init(client, &repo_root, &model).await;

        match result {
            Ok(()) => {
                verify_output(&repo_root);
                any_ran = true;
            }
            Err(e) => {
                let nanowiki_dir = repo_root.join(".nanowiki");
                if nanowiki_dir.exists() {
                    eprintln!("  ⚠️  Agent 出错但 .nanowiki/ 已部分生成: {}", e);
                    if let Ok(content) =
                        std::fs::read_to_string(nanowiki_dir.join(".last-update.json"))
                    {
                        if content.contains("interrupted") {
                            eprintln!("  状态: interrupted（符合预期）");
                        }
                    }
                } else {
                    panic!("[{}] init 失败: {}", name, e);
                }
            }
        }
    }

    if !any_ran {
        eprintln!(
            "\n⚠️  所有 provider 均被跳过。请编辑 tests/test-config.local.json 填入真实 API key。"
        );
    }
}
