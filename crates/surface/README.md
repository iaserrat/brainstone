# surface

Connects the durable runtime to an application interface. Implement
`ThreadSource` to describe a conversation's files and attachments. Implement
`Destinations` to let schedules post into its conversations.

```rust
use brainstone::surface::{Destinations, PostRequest};
use brainstone::runtime::Destination;
use brainstone::agent::ToolError;

struct MySurface;

impl Destinations for MySurface {
    async fn resolve(&self, request: PostRequest<'_>) -> Result<Destination, ToolError> {
        // Map a model-named conversation onto a durable delivery target.
        todo!()
    }
}
```

Everything else in this crate is shared rendering and validation:
`materialize` turns fetched attachments into model input within the caps the
model declares, and the read tools render thread files and thread lists back to
the model.

Returning an error here is a stable failure the model can read; see
[`docs/failure-codes.md`](../../docs/failure-codes.md).
