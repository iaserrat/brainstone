/// The same target configuration is available to the primary and fallback.
pub(crate) struct Names {
    pub(crate) endpoint: &'static str,
    pub(crate) protocol: &'static str,
    pub(crate) target: &'static str,
    pub(crate) auth: &'static str,
    pub(crate) capabilities: &'static str,
    pub(crate) parameters: &'static str,
    pub(crate) headers: &'static str,
    pub(crate) idle: &'static str,
    pub(crate) context: &'static str,
    pub(crate) effort: &'static str,
    pub(crate) output: &'static str,
    pub(crate) reasoning_summary: &'static str,
}

pub(crate) const PRIMARY: Names = Names {
    endpoint: "MODEL_ENDPOINT",
    protocol: "MODEL_PROTOCOL",
    target: "MODEL_TARGET",
    auth: "MODEL_AUTH",
    capabilities: "MODEL_CAPABILITIES",
    headers: "MODEL_HEADERS",
    parameters: "MODEL_PARAMETERS",
    idle: "MODEL_IDLE_TIMEOUT_SECONDS",
    context: "MODEL_CONTEXT_WINDOW",
    effort: "MODEL_EFFORT",
    output: "MODEL_MAX_OUTPUT_TOKENS",
    reasoning_summary: "MODEL_REASONING_SUMMARY",
};
pub(crate) const FALLBACK: Names = Names {
    endpoint: "FALLBACK_MODEL_ENDPOINT",
    protocol: "FALLBACK_MODEL_PROTOCOL",
    target: "FALLBACK_MODEL_TARGET",
    auth: "FALLBACK_MODEL_AUTH",
    capabilities: "FALLBACK_MODEL_CAPABILITIES",
    headers: "FALLBACK_MODEL_HEADERS",
    parameters: "FALLBACK_MODEL_PARAMETERS",
    idle: "FALLBACK_MODEL_IDLE_TIMEOUT_SECONDS",
    context: "FALLBACK_MODEL_CONTEXT_WINDOW",
    effort: "FALLBACK_MODEL_EFFORT",
    output: "FALLBACK_MAX_OUTPUT_TOKENS",
    reasoning_summary: "FALLBACK_MODEL_REASONING_SUMMARY",
};
