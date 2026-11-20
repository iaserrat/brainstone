//! Registration accepts owned typed tools without exposing shared-pointer plumbing.
use brainstone_agent::{Registry, ToolError, typed_tool};
use schemars::JsonSchema;
use serde::Deserialize;
#[derive(Deserialize, JsonSchema)]
struct Args {
    value: u8,
}
#[test]
fn owned_typed_tools_share_one_catalog_and_dispatch_table() {
    let tool = typed_tool("Echo", "Echo a number", |args: Args, _| async move {
        Ok::<_, ToolError>(args.value)
    })
    .unwrap();
    let registry = Registry::default().with_tool(tool);
    assert_eq!(registry.definitions()[0]["name"], "Echo");
    assert_eq!(registry.lookup("Echo").unwrap().name(), "Echo");
}
