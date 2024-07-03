# Block quotes

> This is a simple block quote

> A block quote with more other blocks inside it
>
> ```rust
> fn main() -> eframe::Result {
>     println!("Hello, World!");
> }
> ```

## Alerts

Alerts build upon block quotes.

```markdown
> [!NOTE]
> note alert
```

or

```markdown
> [!NOTE]
>
> note alert
```

will be displayed as:

> [!NOTE]
> note alert

> [!TIP]
> tip alert

<!-- The trailing whitespaces are deliberate on important and warning -->
<!-- Case insensitivity --->
> [!imporTant]
> important alert

> [!WARNING]
> warning alert

> [!CAUTION]
>
> caution alert

The alerts are completely customizable. An arbitrary amount of alerts can be
added
