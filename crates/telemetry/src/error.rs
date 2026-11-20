/// Local telemetry setup failures. Remote exporter failures never reach callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The exporter endpoint, resource, or protocol configuration is unusable.
    Configuration,
    /// A global tracing subscriber is already installed; telemetry installs once.
    AlreadyInstalled,
}
impl Error {
    #[must_use]
    /// The stable code a caller branches on and a log line carries.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Configuration => "telemetry_configuration",
            Self::AlreadyInstalled => "telemetry_already_installed",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for Error {}
