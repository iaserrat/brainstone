use crate::{Config, Transport};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

impl Config {
    /// Fingerprint resolved execution targets without exposing credentials in records.
    /// # Panics
    /// Only if an in-memory JSON configuration cannot be serialized or written.
    #[must_use]
    pub fn execution_binding(&self) -> String {
        let values: Vec<_> = self.servers().iter().map(binding).collect();
        let bytes = serde_json::to_vec(&values).expect("JSON configuration");
        Sha256::digest(bytes)
            .iter()
            .fold(String::new(), |mut text, byte| {
                write!(&mut text, "{byte:02x}").expect("writing string");
                text
            })
    }
}
fn binding(server: &crate::Server) -> Value {
    let target = match server.transport() {
        Transport::Stdio {
            command,
            args,
            env,
            cwd,
        } => json!({"command":command,"args":args,"env":env,"cwd":cwd}),
        Transport::Http {
            url,
            bearer_token,
            headers,
        } => json!({"url":url,"bearer":bearer_token,"headers":headers}),
    };
    json!({"name":server.name(),"target":target,"access":server.access,"tools":server.enabled_tools})
}
