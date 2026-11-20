pub(crate) fn log_diagnostics(catalog: &skills::Catalog) {
    for diagnostic in catalog.diagnostics() {
        tracing::warn!(
            code = diagnostic.code(),
            diagnostic = %diagnostic,
            "skill definition skipped or loaded with a compatibility warning"
        );
    }
}
