# Code blocks

```rs
use egui_commonmark::*;
let markdown =
r"# Hello world

* A list
* [ ] Checkbox
";

let mut cache = CommonMarkCache::default();
CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
```

The `better_syntax_highlighting` feature does not have toml highlighting by
default. It will therefore fallback to default highlighting.

```toml
egui_commonmark = "0.10"
image = { version = "0.24", default-features = false, features = ["png"] }
```

- ```rs
  let x = 3.14;
  ```
- Code blocks can be in lists too :)


More content...

    Inline code blocks are supported if you for some reason need them
