//! Provider Registry
//!
//! Single source of truth for LLM provider metadata.
//! Order matters — it controls match priority and fallback.

use nanobot_config::{ProviderConfig, ProvidersConfig};

/// Provider specification
#[derive(Debug, Clone)]
pub struct ProviderSpec {
    /// Config field name (e.g., "openrouter")
    pub name: &'static str,

    /// Model name keywords for matching (lowercase)
    pub keywords: &'static [&'static str],

    /// Environment variable for API key
    pub env_key: &'static str,

    /// Display name for status output
    pub display_name: &'static str,

    /// Backend implementation type
    pub backend: ProviderBackendType,

    /// Is this a gateway that can route any model?
    pub is_gateway: bool,

    /// Is this a local deployment?
    pub is_local: bool,

    /// OAuth-based provider (no API key)
    pub is_oauth: bool,

    /// Direct provider (user supplies everything)
    pub is_direct: bool,

    /// API key prefix for auto-detection
    pub detect_by_key_prefix: Option<&'static str>,

    /// Base URL keyword for auto-detection
    pub detect_by_base_keyword: Option<&'static str>,

    /// Default API base URL
    pub default_api_base: Option<&'static str>,

    /// Strip provider prefix from model names
    pub strip_model_prefix: bool,

    /// Supports prompt caching (Anthropic)
    pub supports_prompt_caching: bool,
}

/// Backend implementation types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProviderBackendType {
    OpenAiCompat,
    Anthropic,
    AzureOpenai,
    OpenaiCodex,
    GithubCopilot,
}

impl ProviderSpec {
    /// Get the label for display
    pub fn label(&self) -> &str {
        if self.display_name.is_empty() {
            self.name
        } else {
            self.display_name
        }
    }
}

// ============================================================================
// Provider Registry
// ============================================================================

/// All registered providers
/// Order = priority (gateways first for fallback)
pub const PROVIDERS: &[ProviderSpec] = &[
    // === Custom (direct OpenAI-compatible endpoint) ========================
    ProviderSpec {
        name: "custom",
        keywords: &[],
        env_key: "",
        display_name: "Custom",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: true,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: None,
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    // === Azure OpenAI ======================================================
    ProviderSpec {
        name: "azure_openai",
        keywords: &["azure", "azure-openai"],
        env_key: "",
        display_name: "Azure OpenAI",
        backend: ProviderBackendType::AzureOpenai,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: true,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: None,
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    // === Gateways (detected by api_key / api_base) ========================
    ProviderSpec {
        name: "openrouter",
        keywords: &["openrouter"],
        env_key: "OPENROUTER_API_KEY",
        display_name: "OpenRouter",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: true,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: Some("sk-or-"),
        detect_by_base_keyword: Some("openrouter"),
        default_api_base: Some("https://openrouter.ai/api/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: true,
    },

    ProviderSpec {
        name: "aihubmix",
        keywords: &["aihubmix"],
        env_key: "OPENAI_API_KEY",
        display_name: "AiHubMix",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: true,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: Some("aihubmix"),
        default_api_base: Some("https://aihubmix.com/v1"),
        strip_model_prefix: true,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "siliconflow",
        keywords: &["siliconflow"],
        env_key: "OPENAI_API_KEY",
        display_name: "SiliconFlow",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: true,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: Some("siliconflow"),
        default_api_base: Some("https://api.siliconflow.cn/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "volcengine",
        keywords: &["volcengine", "volces", "ark"],
        env_key: "OPENAI_API_KEY",
        display_name: "VolcEngine",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: true,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: Some("volces"),
        default_api_base: Some("https://ark.cn-beijing.volces.com/api/v3"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "volcengine_coding_plan",
        keywords: &["volcengine-plan"],
        env_key: "OPENAI_API_KEY",
        display_name: "VolcEngine Coding Plan",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: true,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://ark.cn-beijing.volces.com/api/coding/v3"),
        strip_model_prefix: true,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "byteplus",
        keywords: &["byteplus"],
        env_key: "OPENAI_API_KEY",
        display_name: "BytePlus",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: true,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: Some("bytepluses"),
        default_api_base: Some("https://ark.ap-southeast.bytepluses.com/api/v3"),
        strip_model_prefix: true,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "byteplus_coding_plan",
        keywords: &["byteplus-plan"],
        env_key: "OPENAI_API_KEY",
        display_name: "BytePlus Coding Plan",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: true,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://ark.ap-southeast.bytepluses.com/api/coding/v3"),
        strip_model_prefix: true,
        supports_prompt_caching: false,
    },

    // === Standard providers (matched by model-name keywords) ===============
    ProviderSpec {
        name: "anthropic",
        keywords: &["anthropic", "claude"],
        env_key: "ANTHROPIC_API_KEY",
        display_name: "Anthropic",
        backend: ProviderBackendType::Anthropic,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: None,
        strip_model_prefix: false,
        supports_prompt_caching: true,
    },

    ProviderSpec {
        name: "openai",
        keywords: &["openai", "gpt"],
        env_key: "OPENAI_API_KEY",
        display_name: "OpenAI",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: None,
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "openai_codex",
        keywords: &["openai-codex"],
        env_key: "",
        display_name: "OpenAI Codex",
        backend: ProviderBackendType::OpenaiCodex,
        is_gateway: false,
        is_local: false,
        is_oauth: true,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: Some("codex"),
        default_api_base: Some("https://chatgpt.com/backend-api"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "github_copilot",
        keywords: &["github_copilot", "copilot"],
        env_key: "",
        display_name: "GitHub Copilot",
        backend: ProviderBackendType::GithubCopilot,
        is_gateway: false,
        is_local: false,
        is_oauth: true,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://api.githubcopilot.com"),
        strip_model_prefix: true,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "deepseek",
        keywords: &["deepseek"],
        env_key: "DEEPSEEK_API_KEY",
        display_name: "DeepSeek",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://api.deepseek.com"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "gemini",
        keywords: &["gemini"],
        env_key: "GEMINI_API_KEY",
        display_name: "Gemini",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://generativelanguage.googleapis.com/v1beta/openai/"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "zhipu",
        keywords: &["zhipu", "glm", "zai"],
        env_key: "ZAI_API_KEY",
        display_name: "Zhipu AI",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://open.bigmodel.cn/api/paas/v4"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "dashscope",
        keywords: &["qwen", "dashscope"],
        env_key: "DASHSCOPE_API_KEY",
        display_name: "DashScope",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "moonshot",
        keywords: &["moonshot", "kimi"],
        env_key: "MOONSHOT_API_KEY",
        display_name: "Moonshot",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://api.moonshot.ai/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "minimax",
        keywords: &["minimax"],
        env_key: "MINIMAX_API_KEY",
        display_name: "MiniMax",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://api.minimax.io/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "mistral",
        keywords: &["mistral"],
        env_key: "MISTRAL_API_KEY",
        display_name: "Mistral",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://api.mistral.ai/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "stepfun",
        keywords: &["stepfun", "step"],
        env_key: "STEPFUN_API_KEY",
        display_name: "Step Fun",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://api.stepfun.com/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    // === Local deployment ==================================================
    ProviderSpec {
        name: "vllm",
        keywords: &["vllm"],
        env_key: "HOSTED_VLLM_API_KEY",
        display_name: "vLLM/Local",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: true,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: None,
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "ollama",
        keywords: &["ollama", "nemotron"],
        env_key: "OLLAMA_API_KEY",
        display_name: "Ollama",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: true,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: Some("11434"),
        default_api_base: Some("http://localhost:11434/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    ProviderSpec {
        name: "ovms",
        keywords: &["openvino", "ovms"],
        env_key: "",
        display_name: "OpenVINO Model Server",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: true,
        is_oauth: false,
        is_direct: true,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("http://localhost:8000/v3"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },

    // === Auxiliary (mainly for Whisper transcription) ======================
    ProviderSpec {
        name: "groq",
        keywords: &["groq"],
        env_key: "GROQ_API_KEY",
        display_name: "Groq",
        backend: ProviderBackendType::OpenAiCompat,
        is_gateway: false,
        is_local: false,
        is_oauth: false,
        is_direct: false,
        detect_by_key_prefix: None,
        detect_by_base_keyword: None,
        default_api_base: Some("https://api.groq.com/openai/v1"),
        strip_model_prefix: false,
        supports_prompt_caching: false,
    },
];

// ============================================================================
// Lookup Helpers
// ============================================================================

/// Find a provider spec by config field name
pub fn find_by_name(name: &str) -> Option<&'static ProviderSpec> {
    PROVIDERS.iter().find(|spec| spec.name == name)
}

/// Find a provider spec by model name
pub fn find_by_model_name(model: &str) -> Option<&'static ProviderSpec> {
    let model_lower = model.to_lowercase();
    let model_normalized = model_lower.replace('-', "_");
    let model_prefix = model_lower.split('/').next().unwrap_or("");
    let normalized_prefix = model_prefix.replace('-', "_");

    // Explicit provider prefix match first
    for spec in PROVIDERS {
        if !spec.keywords.is_empty() && normalized_prefix == spec.name {
            return Some(spec);
        }
    }

    // Keyword match
    for spec in PROVIDERS {
        for &keyword in spec.keywords {
            let kw_lower = keyword.to_lowercase();
            if model_lower.contains(&kw_lower) || model_normalized.contains(&kw_lower.replace('-', "_")) {
                return Some(spec);
            }
        }
    }

    None
}

/// Match provider to config and model
pub fn match_provider<'a>(
    config: &'a ProvidersConfig,
    model: Option<&'a str>,
    forced_provider: &'a str,
) -> Option<(&'a ProviderConfig, &'static ProviderSpec)> {
    // Forced provider
    if forced_provider != "auto" {
        if let Some(spec) = find_by_name(forced_provider) {
            return get_provider_config(config, spec.name).map(|c| (c, spec));
        }
        return None;
    }

    // Auto-detect by model name
    let model_str = model.unwrap_or("anthropic/claude-opus-4-5");
    if let Some(spec) = find_by_model_name(model_str) {
        return get_provider_config(config, spec.name).map(|c| (c, spec));
    }

    // Fallback to configured gateways
    for spec in PROVIDERS {
        if spec.is_gateway || spec.is_local {
            if let Some(provider_config) = get_provider_config(config, spec.name) {
                if !provider_config.api_key.is_empty() || spec.is_oauth || spec.is_direct {
                    return Some((provider_config, spec));
                }
            }
        }
    }

    None
}

fn get_provider_config<'a>(config: &'a ProvidersConfig, name: &str) -> Option<&'a ProviderConfig> {
    match name {
        "custom" => Some(&config.custom),
        "anthropic" => Some(&config.anthropic),
        "openai" => Some(&config.openai),
        "openrouter" => Some(&config.openrouter),
        "azure_openai" => Some(&config.azure_openai),
        "deepseek" => Some(&config.deepseek),
        "groq" => Some(&config.groq),
        "zhipu" => Some(&config.zhipu),
        "dashscope" => Some(&config.dashscope),
        "vllm" => Some(&config.vllm),
        "ollama" => Some(&config.ollama),
        "ovms" => Some(&config.ovms),
        "gemini" => Some(&config.gemini),
        "moonshot" => Some(&config.moonshot),
        "minimax" => Some(&config.minimax),
        "mistral" => Some(&config.mistral),
        "stepfun" => Some(&config.stepfun),
        "aihubmix" => Some(&config.aihubmix),
        "siliconflow" => Some(&config.siliconflow),
        "volcengine" => Some(&config.volcengine),
        "volcengine_coding_plan" => Some(&config.volcengine_coding_plan),
        "byteplus" => Some(&config.byteplus),
        "byteplus_coding_plan" => Some(&config.byteplus_coding_plan),
        "openai_codex" => Some(&config.openai_codex),
        "github_copilot" => Some(&config.github_copilot),
        _ => None,
    }
}
