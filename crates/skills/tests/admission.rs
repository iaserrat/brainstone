#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
use std::path::Path;
use std::sync::{Arc, Mutex};

use agent::{Image, ImageMediaType};
use brainstone_skills::{Catalog, SkillAdmission};
use host::Admission;
use runtime::{Address, OperationId, Prompt, PromptContent};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Option<Prompt>>>);

impl Admission for Capture {
    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, host::Error> {
        self.0.lock().expect("capture lock").replace(prompt.clone());
        Ok(OperationId::new(1).expect("operation id"))
    }
}

#[tokio::test]
async fn image_only_prompts_pass_through_skill_admission() {
    let capture = Capture::default();
    let admission = SkillAdmission::new(capture.clone(), Catalog::load(Path::new("missing")));
    let address = Address::new("test", "conversation", "lane").unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1], 1, 1).unwrap();
    let content = PromptContent {
        images: vec![image],
        ..PromptContent::default()
    };
    let prompt = Prompt::with_content(address, "request-image", "system", content).unwrap();

    admission.accept(&prompt).await.unwrap();

    let accepted = capture.0.lock().unwrap().clone().expect("accepted prompt");
    assert_eq!(accepted.text(), "");
    assert_eq!(accepted.images(), prompt.images());
}

#[tokio::test]
async fn explicit_skill_mentions_are_injected_before_durable_admission() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
    let capture = Capture::default();
    let admission = SkillAdmission::new(capture.clone(), Catalog::load(&root));
    let address = Address::new("test", "conversation", "lane").unwrap();
    let prompt = Prompt::new(
        address,
        "request-1",
        "system",
        "Please use $report-filing for this file; $report-filing is explicit.",
    )
    .unwrap();

    admission.accept(&prompt).await.unwrap();

    let accepted = capture.0.lock().unwrap().clone().expect("accepted prompt");
    assert!(accepted.text().contains("# Report filing"));
    assert!(
        accepted
            .text()
            .contains("<skill_content name=\"report-filing\">")
    );
    assert_eq!(accepted.text().matches("# Report filing").count(), 1);
    assert!(
        accepted
            .text()
            .ends_with("Please use $report-filing for this file; $report-filing is explicit.")
    );
}
