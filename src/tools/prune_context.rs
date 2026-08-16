//! PruneContext tool — 清理对话上下文，分阶段控制上下文大小

/// 返回确认消息。真正的剪枝逻辑在 AgentRunner::run 中处理。
pub fn prune_context(summary: &str, plan: &str) -> Result<String, String> {
    Ok(format!(
        "上下文已清理。\n\n## 已完成\n{}\n\n## 后续计划\n{}",
        summary, plan
    ))
}
