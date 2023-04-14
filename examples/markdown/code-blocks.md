# Code blocks


```toml
egui_commonmark = "0.10"
image = { version = "0.24", default-features = false, features = ["png"] }
```

```rust
use egui_commonmark::*;
let markdown =
r"# Hello world

* A list
* [ ] Checkbox
";

let mut cache = CommonMarkCache::default();
CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
```
