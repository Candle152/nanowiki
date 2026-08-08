//! 格式化工具 — 将数据按指定格式输出

/// 根据格式类型格式化原始数据
pub fn format_output(data: &str, format: &str) -> Result<String, String> {
    match format {
        "json" => {
            // 验证是否为合法 JSON，是则美化，否则包裹
            if serde_json::from_str::<serde_json::Value>(data).is_ok() {
                let pretty = serde_json::to_string_pretty(
                    &serde_json::from_str::<serde_json::Value>(data).unwrap(),
                )
                .map_err(|e| e.to_string())?;
                Ok(pretty)
            } else {
                let wrapped = serde_json::json!({ "raw": data });
                serde_json::to_string_pretty(&wrapped).map_err(|e| e.to_string())
            }
        }
        "toml" => {
            // 简单处理：如果不是 TOML 就报错
            if data.trim().starts_with('[') || data.contains('=') {
                Ok(data.to_string())
            } else {
                Err("数据不是有效的 TOML 格式".into())
            }
        }
        "yaml" => Err("YAML 格式暂未支持".into()),
        other => Err(format!("不支持的格式: {}", other)),
    }
}
