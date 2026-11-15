//! A registry holds fixed tools and sources that list their tools again.

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::{Registry, Tool, ToolError, ToolOutput, ToolRequest, ToolSource};

/// A tool known only by its name.
struct Named(&'static str);

impl Tool for Named {
    fn name(&self) -> &str {
        self.0
    }

    fn definition(&self) -> Value {
        json!({"name": self.0})
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async { Ok(String::new().into()) })
    }
}

/// Lists `next` on refresh. A distinct name per tool is the point here, so the
/// fixed-name `Echo` double in `run_tests.rs` does not fit.
struct Listing {
    listed: Mutex<Vec<Arc<dyn Tool>>>,
    next: Mutex<Option<Vec<Arc<dyn Tool>>>>,
}

impl ToolSource for Listing {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.listed.lock().unwrap().clone()
    }

    fn refresh(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        if let Some(next) = self.next.lock().unwrap().take() {
            *self.listed.lock().unwrap() = next;
        }
        Box::pin(async {})
    }
}

fn named(name: &'static str) -> Arc<dyn Tool> {
    Arc::new(Named(name))
}

fn names(registry: &Registry) -> Vec<Value> {
    registry
        .definitions()
        .into_iter()
        .map(|definition| definition["name"].clone())
        .collect()
}

#[tokio::test]
async fn a_registry_offers_what_its_sources_listed_last() {
    let source = Arc::new(Listing {
        listed: Mutex::new(vec![named("search")]),
        next: Mutex::new(Some(vec![named("search"), named("create")])),
    });
    let registry = Registry::from_sources(vec![source.clone()]).extend(vec![named("reply")]);
    assert_eq!(names(&registry), [json!("search"), json!("reply")]);

    let listed = registry.listed().await;

    assert_eq!(
        names(&listed),
        [json!("search"), json!("create"), json!("reply")]
    );
    assert!(listed.lookup("create").is_some());
    *source.next.lock().unwrap() = Some(vec![named("other")]);
    source.refresh().await;
    assert_eq!(names(&registry), [json!("other"), json!("reply")]);
    assert_eq!(
        names(&listed),
        [json!("search"), json!("create"), json!("reply")]
    );
}
