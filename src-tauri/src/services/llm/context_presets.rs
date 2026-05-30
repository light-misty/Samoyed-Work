//! 模型上下文窗口预设表
//! 内置主流大模型的上下文窗口大小数据，支持按模型名称模糊匹配
//! 数据基于 2025-2026 年公开信息整理，用于自动推断模型上下文窗口大小

/// 模型上下文窗口预设项
pub struct ContextPreset {
    /// 模型名称关键词（用于模糊匹配，需小写）
    pub model_pattern: &'static str,
    /// 上下文窗口大小 (tokens)
    pub context_window: usize,
    /// Provider 类型（可选，用于精确匹配）
    pub provider_type: Option<&'static str>,
}

/// 兜底默认上下文窗口大小
const DEFAULT_CONTEXT_WINDOW: usize = 128_000;

/// Ollama 保守默认值
const OLLAMA_DEFAULT_CONTEXT_WINDOW: usize = 8192;

/// 内置模型预设表
/// 排列顺序影响匹配优先级：更具体的模式应排在更前面
/// 同一 provider_type 下，长模式优先于短模式（如 "gpt-4.1" 在 "gpt-4" 前面）
static CONTEXT_PRESETS: &[ContextPreset] = &[
    // ================================================================
    // OpenAI 官方模型（provider_type = "openai"，仅匹配以 gpt-/o3/o4 开头的模型）
    // ================================================================
    ContextPreset { model_pattern: "gpt-5.5", context_window: 1_000_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-5.4-mini", context_window: 400_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-5.4-nano", context_window: 400_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-5.4", context_window: 272_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-4.1-mini", context_window: 1_000_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-4.1-nano", context_window: 1_000_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-4.1", context_window: 1_000_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-4o-mini", context_window: 128_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-4o", context_window: 128_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-4-turbo", context_window: 128_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-4", context_window: 128_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "gpt-3.5-turbo", context_window: 16_385, provider_type: Some("openai") },
    ContextPreset { model_pattern: "o4-mini", context_window: 200_000, provider_type: Some("openai") },
    ContextPreset { model_pattern: "o3", context_window: 200_000, provider_type: Some("openai") },

    // ================================================================
    // Anthropic 官方模型（provider_type = "anthropic"）
    // ================================================================
    ContextPreset { model_pattern: "claude-opus-4-7", context_window: 2_000_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-opus-4-6", context_window: 1_000_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-sonnet-4-6", context_window: 1_000_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-haiku-4-5", context_window: 200_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-3-7-sonnet", context_window: 200_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-3-5-sonnet", context_window: 200_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-3-5-haiku", context_window: 200_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-3-opus", context_window: 200_000, provider_type: Some("anthropic") },
    ContextPreset { model_pattern: "claude-3-haiku", context_window: 200_000, provider_type: Some("anthropic") },
    // Anthropic 通用匹配（claude 开头但未精确匹配的）
    ContextPreset { model_pattern: "claude", context_window: 200_000, provider_type: Some("anthropic") },

    // ================================================================
    // Google Gemini 官方模型（provider_type = "gemini"）
    // ================================================================
    ContextPreset { model_pattern: "gemini-3.1-pro", context_window: 1_000_000, provider_type: Some("gemini") },
    ContextPreset { model_pattern: "gemini-3.5-flash", context_window: 128_000, provider_type: Some("gemini") },
    ContextPreset { model_pattern: "gemini-2.5-pro", context_window: 1_000_000, provider_type: Some("gemini") },
    ContextPreset { model_pattern: "gemini-2.5-flash-lite", context_window: 1_000_000, provider_type: Some("gemini") },
    ContextPreset { model_pattern: "gemini-2.5-flash", context_window: 1_000_000, provider_type: Some("gemini") },
    ContextPreset { model_pattern: "gemini-2.0-flash", context_window: 1_000_000, provider_type: Some("gemini") },
    ContextPreset { model_pattern: "gemini-1.5-pro", context_window: 2_000_000, provider_type: Some("gemini") },
    ContextPreset { model_pattern: "gemini-1.5-flash", context_window: 1_000_000, provider_type: Some("gemini") },
    // Gemini 通用匹配
    ContextPreset { model_pattern: "gemini", context_window: 1_000_000, provider_type: Some("gemini") },

    // ================================================================
    // 以下模型通过 OpenAI 兼容 API 访问，provider_type 为 "openai" 或 "custom"
    // 不设置 provider_type，仅按模型名称匹配
    // ================================================================

    // DeepSeek
    ContextPreset { model_pattern: "deepseek-v4-pro", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "deepseek-v4-flash", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "deepseek-r2", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "deepseek-v3", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "deepseek-r1", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "deepseek", context_window: 128_000, provider_type: None },

    // Meta Llama
    ContextPreset { model_pattern: "llama-4-scout", context_window: 10_000_000, provider_type: None },
    ContextPreset { model_pattern: "llama-4-maverick", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "llama-3.3-70b", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "llama-3.1-405b", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "llama-3.1-70b", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "llama-3.1-8b", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "llama", context_window: 128_000, provider_type: None },

    // 阿里云 Qwen
    ContextPreset { model_pattern: "qwen3.7-max", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "qwen3.6-plus", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "qwen3.6-max", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "qwen3-235b", context_window: 262_144, provider_type: None },
    ContextPreset { model_pattern: "qwen3-30b", context_window: 262_144, provider_type: None },
    ContextPreset { model_pattern: "qwen3-14b", context_window: 131_072, provider_type: None },
    ContextPreset { model_pattern: "qwen2.5-1m", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "qwen-max", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "qwen-plus", context_window: 131_072, provider_type: None },
    ContextPreset { model_pattern: "qwen-turbo", context_window: 1_000_000, provider_type: None },
    ContextPreset { model_pattern: "qwen", context_window: 128_000, provider_type: None },

    // 月之暗面 Kimi
    ContextPreset { model_pattern: "kimi-k2-6", context_window: 262_144, provider_type: None },
    ContextPreset { model_pattern: "kimi-k2-5", context_window: 256_000, provider_type: None },
    ContextPreset { model_pattern: "kimi-k2", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "moonshot-v1-128k", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "moonshot-v1-32k", context_window: 32_000, provider_type: None },
    ContextPreset { model_pattern: "moonshot", context_window: 128_000, provider_type: None },

    // 智谱AI GLM
    ContextPreset { model_pattern: "glm-5.1", context_window: 200_000, provider_type: None },
    ContextPreset { model_pattern: "glm-4-flash", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "glm-4", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "glm", context_window: 128_000, provider_type: None },

    // 百度 ERNIE
    ContextPreset { model_pattern: "ernie-5.1", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "ernie-4.0", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "ernie-3.5", context_window: 32_000, provider_type: None },
    ContextPreset { model_pattern: "ernie", context_window: 128_000, provider_type: None },

    // 字节跳动 Doubao/Seed
    ContextPreset { model_pattern: "seed-2.0-pro", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "doubao-1.5-pro", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "doubao-1.5-lite", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "doubao", context_window: 128_000, provider_type: None },

    // MiniMax
    ContextPreset { model_pattern: "minimax-m2.7", context_window: 200_000, provider_type: None },
    ContextPreset { model_pattern: "minimax", context_window: 128_000, provider_type: None },

    // 零一万物 Yi
    ContextPreset { model_pattern: "yi-large", context_window: 200_000, provider_type: None },
    ContextPreset { model_pattern: "yi-lightning", context_window: 16_000, provider_type: None },
    ContextPreset { model_pattern: "yi", context_window: 128_000, provider_type: None },

    // 百川
    ContextPreset { model_pattern: "baichuan-4", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "baichuan", context_window: 128_000, provider_type: None },

    // 讯飞星火
    ContextPreset { model_pattern: "spark-v4", context_window: 32_000, provider_type: None },
    ContextPreset { model_pattern: "spark", context_window: 32_000, provider_type: None },

    // Mistral
    ContextPreset { model_pattern: "mistral-large-3", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "mistral-small-4", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "magistral", context_window: 128_000, provider_type: None },
    ContextPreset { model_pattern: "mistral", context_window: 128_000, provider_type: None },

    // 腾讯混元
    ContextPreset { model_pattern: "hunyuan", context_window: 128_000, provider_type: None },
];

/// 判断模型名称是否为 OpenAI 官方模型
/// OpenAI 官方模型以 gpt- 或 o3/o4 开头
/// 用于区分通过 OpenAI 兼容 API 访问的非 OpenAI 模型（如 DeepSeek、Qwen 等）
fn is_openai_official_model(model_name: &str) -> bool {
    let lower = model_name.to_lowercase();
    lower.starts_with("gpt-") || lower.starts_with("o3") || lower.starts_with("o4")
}

/// 根据模型名称和 Provider 类型推断上下文窗口大小
///
/// 匹配优先级:
/// 1. 精确 provider_type + model_pattern: 如 provider_type = "openai" + model_pattern = "gpt-4.1"
/// 2. 仅 model_pattern 精确匹配: 如模型名含 "gpt-4.1" 精确匹配 1M
/// 3. model_pattern 模糊匹配: 如模型名含 "gpt-4" 匹配 128K
/// 4. 兜底默认值 128K
///
/// 特殊处理:
/// - Ollama Provider 返回保守默认值 8192
/// - OpenAI 兼容 API 的 Provider (provider_type = "openai") 需额外检查模型名是否为 OpenAI 官方模型
pub fn resolve_context_window(model_name: &str, provider_type: Option<&str>) -> usize {
    let model_lower = model_name.to_lowercase();

    // Ollama 特殊处理：返回保守默认值
    if provider_type == Some("ollama") {
        log::debug!(
            "Ollama Provider，使用保守默认上下文窗口: {} (模型: {})",
            OLLAMA_DEFAULT_CONTEXT_WINDOW, model_name
        );
        return OLLAMA_DEFAULT_CONTEXT_WINDOW;
    }

    // OpenAI 兼容 API 的特殊处理：
    // 如果 provider_type 为 "openai" 但模型名不是 OpenAI 官方模型，
    // 则跳过带 provider_type = "openai" 的预设项，仅按模型名称匹配
    let skip_openai_presets = provider_type == Some("openai") && !is_openai_official_model(model_name);

    // 第一轮：精确 provider_type + model_pattern 匹配
    if !skip_openai_presets {
        if let Some(provider) = provider_type {
            for preset in CONTEXT_PRESETS {
                if let Some(pt) = preset.provider_type {
                    if pt == provider && model_lower.contains(preset.model_pattern) {
                        log::debug!(
                            "精确匹配上下文窗口: {} tokens (provider={}, pattern={})",
                            preset.context_window, provider, preset.model_pattern
                        );
                        return preset.context_window;
                    }
                }
            }
        }
    }

    // 第二轮：仅按 model_pattern 匹配（provider_type 为 None 的预设项）
    // 按模式长度降序匹配，更具体的模式优先
    let mut best_match: Option<&ContextPreset> = None;
    let mut best_pattern_len = 0;

    for preset in CONTEXT_PRESETS {
        if preset.provider_type.is_some() {
            continue; // 跳过带 provider_type 的预设，已在第一轮处理
        }
        if model_lower.contains(preset.model_pattern) {
            // 选择最长的匹配模式（更具体）
            if preset.model_pattern.len() > best_pattern_len {
                best_pattern_len = preset.model_pattern.len();
                best_match = Some(preset);
            }
        }
    }

    if let Some(preset) = best_match {
        log::debug!(
            "模型名称匹配上下文窗口: {} tokens (pattern={})",
            preset.context_window, preset.model_pattern
        );
        return preset.context_window;
    }

    // 兜底默认值
    log::debug!(
        "未匹配到预设，使用默认上下文窗口: {} tokens (模型: {})",
        DEFAULT_CONTEXT_WINDOW, model_name
    );
    DEFAULT_CONTEXT_WINDOW
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_gpt4o() {
        assert_eq!(resolve_context_window("gpt-4o", Some("openai")), 128_000);
    }

    #[test]
    fn test_openai_gpt41() {
        assert_eq!(resolve_context_window("gpt-4.1", Some("openai")), 1_000_000);
    }

    #[test]
    fn test_openai_gpt41_mini() {
        assert_eq!(resolve_context_window("gpt-4.1-mini", Some("openai")), 1_000_000);
    }

    #[test]
    fn test_openai_o3() {
        assert_eq!(resolve_context_window("o3", Some("openai")), 200_000);
    }

    #[test]
    fn test_openai_o4_mini() {
        assert_eq!(resolve_context_window("o4-mini", Some("openai")), 200_000);
    }

    #[test]
    fn test_openai_gpt35_turbo() {
        assert_eq!(resolve_context_window("gpt-3.5-turbo", Some("openai")), 16_385);
    }

    #[test]
    fn test_anthropic_claude_3_5_sonnet() {
        assert_eq!(resolve_context_window("claude-3-5-sonnet-20241022", Some("anthropic")), 200_000);
    }

    #[test]
    fn test_anthropic_claude_opus_4_7() {
        assert_eq!(resolve_context_window("claude-opus-4-7-20260501", Some("anthropic")), 2_000_000);
    }

    #[test]
    fn test_anthropic_claude_fallback() {
        // 未精确匹配的 claude 模型，应匹配通用 claude 模式
        assert_eq!(resolve_context_window("claude-some-new-model", Some("anthropic")), 200_000);
    }

    #[test]
    fn test_gemini_2_5_pro() {
        assert_eq!(resolve_context_window("gemini-2.5-pro", Some("gemini")), 1_000_000);
    }

    #[test]
    fn test_gemini_1_5_pro() {
        assert_eq!(resolve_context_window("gemini-1.5-pro", Some("gemini")), 2_000_000);
    }

    #[test]
    fn test_ollama_default() {
        assert_eq!(resolve_context_window("llama3", Some("ollama")), 8192);
        assert_eq!(resolve_context_window("qwen2", Some("ollama")), 8192);
    }

    #[test]
    fn test_openai_compatible_deepseek() {
        // DeepSeek 通过 OpenAI 兼容 API 访问，provider_type 为 "openai"
        // 但模型名不是 OpenAI 官方模型，应按模型名称匹配而非 OpenAI 预设
        assert_eq!(resolve_context_window("deepseek-v3", Some("openai")), 128_000);
        assert_eq!(resolve_context_window("deepseek-r1", Some("openai")), 128_000);
    }

    #[test]
    fn test_openai_compatible_qwen() {
        // Qwen 通过 OpenAI 兼容 API 访问
        assert_eq!(resolve_context_window("qwen-max", Some("openai")), 1_000_000);
    }

    #[test]
    fn test_custom_provider_deepseek() {
        // custom Provider，完全依赖模型名称匹配
        assert_eq!(resolve_context_window("deepseek-v3", Some("custom")), 128_000);
        assert_eq!(resolve_context_window("deepseek-r2", Some("custom")), 1_000_000);
    }

    #[test]
    fn test_custom_provider_llama() {
        assert_eq!(resolve_context_window("llama-4-scout", Some("custom")), 10_000_000);
        assert_eq!(resolve_context_window("llama-3.1-70b", Some("custom")), 128_000);
    }

    #[test]
    fn test_no_provider_type() {
        // 无 provider_type，仅按模型名称匹配
        assert_eq!(resolve_context_window("deepseek-v3", None), 128_000);
        assert_eq!(resolve_context_window("qwen-max", None), 1_000_000);
    }

    #[test]
    fn test_unknown_model_fallback() {
        assert_eq!(resolve_context_window("some-unknown-model", None), DEFAULT_CONTEXT_WINDOW);
        assert_eq!(resolve_context_window("unknown", Some("custom")), DEFAULT_CONTEXT_WINDOW);
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(resolve_context_window("GPT-4O", Some("openai")), 128_000);
        assert_eq!(resolve_context_window("Claude-3-5-Sonnet", Some("anthropic")), 200_000);
        assert_eq!(resolve_context_window("DeepSeek-V3", Some("custom")), 128_000);
    }

    #[test]
    fn test_model_name_with_version_suffix() {
        // 模型名可能带版本后缀，如日期
        assert_eq!(resolve_context_window("gpt-4o-2024-05-13", Some("openai")), 128_000);
        assert_eq!(resolve_context_window("claude-3-5-sonnet-20241022", Some("anthropic")), 200_000);
    }

    #[test]
    fn test_kimi_models() {
        assert_eq!(resolve_context_window("kimi-k2-6", Some("custom")), 262_144);
        assert_eq!(resolve_context_window("moonshot-v1-128k", Some("custom")), 128_000);
    }

    #[test]
    fn test_glm_models() {
        assert_eq!(resolve_context_window("glm-5.1", Some("custom")), 200_000);
        assert_eq!(resolve_context_window("glm-4", Some("custom")), 128_000);
    }

    #[test]
    fn test_is_openai_official_model() {
        assert!(is_openai_official_model("gpt-4o"));
        assert!(is_openai_official_model("gpt-4.1"));
        assert!(is_openai_official_model("o3"));
        assert!(is_openai_official_model("o4-mini"));
        assert!(is_openai_official_model("GPT-4O"));
        assert!(!is_openai_official_model("deepseek-v3"));
        assert!(!is_openai_official_model("qwen-max"));
        assert!(!is_openai_official_model("claude-3"));
    }

    #[test]
    fn test_qwen_plus_128k() {
        // qwen-plus 是 128K（131072），不是 1M
        assert_eq!(resolve_context_window("qwen-plus", Some("custom")), 131_072);
    }

    #[test]
    fn test_doubao_models() {
        assert_eq!(resolve_context_window("doubao-1.5-pro", Some("custom")), 128_000);
        assert_eq!(resolve_context_window("seed-2.0-pro", Some("custom")), 128_000);
    }

    #[test]
    fn test_mistral_models() {
        assert_eq!(resolve_context_window("mistral-large-3", Some("custom")), 128_000);
        assert_eq!(resolve_context_window("magistral", Some("custom")), 128_000);
    }
}
