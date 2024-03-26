## Parsing backends

egui_commonmark offers __experimental__ support for using [comrak](https://crates.io/crates/comrak)
as parsing backend. By default pulldown_cmark is used. To use comrak instead do:

```toml
egui_commonmark = { version = "0.14", default-features = false, features = ["load-images", "comrak"] }
```

### Why two parsing backends?

egui_commonmark was originally implemented using pulldown_cmark, however comrak
has stricter commonmark/github style markdown support. In addition it allows the
crate to support more features than pulldown_cmark can offer with regards to github
style markdown.

pulldown_cmark has fewer dependencies and should theoretically be faster.

Due to these reasons both backends are supported. At least for now.
If you are unsure of what to use, just use pulldown_cmark the default backend.


### Differences in support

The comrak backend does not support Alerts due do them being difficult to
implement with comrak

### Known rendering differences

| Type    | pulldown_cmark | comrak |
|---------|----------------|--------|
| Footers | Placed when they appear | Placed at the end |
| Spec incompatibilities | Blocks such as images can be rendered inside tables. This is against the spec | Disallowed |

