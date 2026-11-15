use anthropic::Effort;

/// `MODEL_EFFORT`'s five levels exist under the same names in both protocols.
pub(crate) const fn openai_effort(effort: Effort) -> openai::Effort {
    match effort {
        Effort::Low => openai::Effort::Low,
        Effort::Medium => openai::Effort::Medium,
        Effort::High => openai::Effort::High,
        Effort::XHigh => openai::Effort::XHigh,
        Effort::Max => openai::Effort::Max,
    }
}
