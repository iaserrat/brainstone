pub(crate) fn install() {
    let provider = rustls::crypto::ring::default_provider();
    drop(provider.install_default());
}
