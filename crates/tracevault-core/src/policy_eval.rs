//! Shared policy evaluation logic used by both the CLI check flow (via the
//! server) and the server's check endpoint. Single source of truth so glob
//! semantics and tool-name matching can't drift.

use std::collections::HashMap;

pub struct EvalOutcome {
    pub passed: bool,
    pub details: String,
}

/// Evaluate a condition JSON document against aggregated session data.
///
/// `tool_calls` maps tool name → total call count across all sessions in the
/// push. `files_modified` is the union of file paths touched.
pub fn evaluate_condition(
    condition: &serde_json::Value,
    tool_calls: &HashMap<String, i64>,
    files_modified: &[String],
) -> EvalOutcome {
    let cond_type = condition.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match cond_type {
        "RequiredToolCall" => eval_required(condition, tool_calls),
        "ConditionalToolCall" => eval_conditional(condition, tool_calls, files_modified),
        "AiPercentageThreshold" => EvalOutcome {
            passed: true,
            details: "AI percentage not evaluated in check (requires attribution data)".into(),
        },
        "TokenBudget" => EvalOutcome {
            passed: true,
            details: "Token budget not evaluated in check (requires token data)".into(),
        },
        _ => EvalOutcome {
            passed: true,
            details: format!("Unknown condition type '{cond_type}', skipped"),
        },
    }
}

fn eval_required(condition: &serde_json::Value, tool_calls: &HashMap<String, i64>) -> EvalOutcome {
    let tool_names: Vec<String> = condition
        .get("tool_names")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let missing: Vec<&String> = tool_names
        .iter()
        .filter(|name| tool_calls.get(name.as_str()).copied().unwrap_or(0) == 0)
        .collect();

    if missing.is_empty() {
        EvalOutcome {
            passed: true,
            details: "All required tools were used".into(),
        }
    } else {
        EvalOutcome {
            passed: false,
            details: format!(
                "Missing required tools: {}",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

fn eval_conditional(
    condition: &serde_json::Value,
    tool_calls: &HashMap<String, i64>,
    files_modified: &[String],
) -> EvalOutcome {
    let tool_name = condition
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let min_count = condition
        .get("min_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as i64;
    let file_patterns = condition
        .get("when_files_match")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        });

    let patterns_match = match &file_patterns {
        None => true,
        Some(patterns) if patterns.is_empty() => true,
        Some(patterns) => files_modified.iter().any(|file| {
            patterns
                .iter()
                .any(|pattern| glob_match::glob_match(pattern, file))
        }),
    };

    if !patterns_match {
        return EvalOutcome {
            passed: true,
            details: "Rule skipped: no modified files match patterns".into(),
        };
    }

    let actual_count = tool_calls.get(tool_name).copied().unwrap_or(0);

    let passed = actual_count >= min_count;
    EvalOutcome {
        passed,
        details: format!(
            "Tool '{tool_name}' called {actual_count} time(s) (required >= {min_count})"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tools(pairs: &[(&str, i64)]) -> HashMap<String, i64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn required_all_present_exact_match() {
        let cond = json!({"type": "RequiredToolCall", "tool_names": ["Read", "Write"]});
        let t = tools(&[("Read", 1), ("Write", 1)]);
        assert!(evaluate_condition(&cond, &t, &[]).passed);
    }

    #[test]
    fn required_substring_does_not_match() {
        // Previously `Read` matched `read_file` via substring. Now: exact only.
        let cond = json!({"type": "RequiredToolCall", "tool_names": ["Read"]});
        let t = tools(&[("read_file", 5)]);
        assert!(!evaluate_condition(&cond, &t, &[]).passed);
    }

    #[test]
    fn required_missing_reports_names() {
        let cond = json!({"type": "RequiredToolCall", "tool_names": ["Lint"]});
        let out = evaluate_condition(&cond, &HashMap::new(), &[]);
        assert!(!out.passed);
        assert!(out.details.contains("Lint"));
    }

    #[test]
    fn conditional_glob_file_match_count_met() {
        let cond = json!({
            "type": "ConditionalToolCall",
            "tool_name": "security_scan",
            "min_count": 1,
            "when_files_match": ["**/*.rs"]
        });
        let t = tools(&[("security_scan", 2)]);
        let files = vec!["src/main.rs".to_string()];
        assert!(evaluate_condition(&cond, &t, &files).passed);
    }

    #[test]
    fn conditional_count_not_met_fails() {
        let cond = json!({
            "type": "ConditionalToolCall",
            "tool_name": "security_scan",
            "min_count": 5,
            "when_files_match": ["**/*.rs"]
        });
        let t = tools(&[("security_scan", 1)]);
        let files = vec!["src/main.rs".to_string()];
        assert!(!evaluate_condition(&cond, &t, &files).passed);
    }

    #[test]
    fn conditional_no_file_match_skips() {
        let cond = json!({
            "type": "ConditionalToolCall",
            "tool_name": "security_scan",
            "min_count": 1,
            "when_files_match": ["*.py"]
        });
        let files = vec!["src/main.rs".to_string()];
        let out = evaluate_condition(&cond, &HashMap::new(), &files);
        assert!(out.passed);
        assert!(out.details.contains("skipped"));
    }

    #[test]
    fn conditional_no_patterns_always_applies() {
        let cond = json!({
            "type": "ConditionalToolCall",
            "tool_name": "test",
            "min_count": 1
        });
        let out = evaluate_condition(&cond, &HashMap::new(), &[]);
        assert!(!out.passed);
    }

    #[test]
    fn conditional_tool_name_exact_only() {
        // Previously `security_scan` matched `my_security_scan_v2` via substring.
        let cond = json!({
            "type": "ConditionalToolCall",
            "tool_name": "security_scan",
            "min_count": 1
        });
        let t = tools(&[("my_security_scan_v2", 10)]);
        assert!(!evaluate_condition(&cond, &t, &[]).passed);
    }

    #[test]
    fn unknown_condition_passes() {
        let cond = json!({"type": "FutureCondition"});
        assert!(evaluate_condition(&cond, &HashMap::new(), &[]).passed);
    }
}
