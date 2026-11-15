# surface

The seam between the durable runtime and whatever interface you put in front of
it. A surface implements `ThreadSource` to describe the files and attachments a
conversation carries, and `Destinations` to let a schedule post into its
conversations.

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
