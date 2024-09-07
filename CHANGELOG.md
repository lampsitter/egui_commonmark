# egui_commomnark changelog

## Unreleased

### Added

- Definition lists
- Proper inline code block rendering

### Changed

- `CommonMarkViewer::new` no longer takes in an id.
- `commonmark!` and `commonmark_str!` no longer takes in an id.
- `CommonMarkViewer::show_scrollable` takes in an id explicity.

- Updated pulldown-cmark to 0.12
- Newlines are no longer inserted before/after markdown ([#56](https://github.com/lampsitter/egui_commonmark/pull/56))
    > For the old behaviour you can call `ui.label("");` before and and after

### Removed

- Experimental comrak backend ([#57](https://github.com/lampsitter/egui_commonmark/pull/57))
- Deprecated method `syntax_theme`

## 0.17.0 - 2024-07-03

### Changed

- Updated egui to 0.28 ([#51](https://github.com/lampsitter/egui_commonmark/pull/51) by [@emilk](https://github.com/emilk))
- Updated pulldown-cmark to 0.11

## 0.16.1 - 2024-05-12

## Fixed

- Fixed docs.rs build

## 0.16.0 - 2024-05-12

### Added

- `commonmark!` and `commonmark_str!` for compile time parsing of markdown. The
  proc macros will output egui widgets directly into your code. To use this
  enable the `macros` feature.

### Changed

- MSRV bumped to 1.76

### Fixed

- Missing newline before alerts

## 0.15.0 - 2024-04-02

### Added

- Replace copy icon with checkmark when clicking copy button in code blocks
([#42](https://github.com/lampsitter/egui_commonmark/pull/42) by [@zeozeozeo](https://github.com/zeozeozeo))
- Interactive tasklists with `CommonMarkViewer::show_mut`
([#40](https://github.com/lampsitter/egui_commonmark/pull/40) by [@crumblingstatue](https://github.com/crumblingstatue))

### Changed

- MSRV bumped to 1.74 due to pulldown_cmark
- Alerts are case-insensitive
- More spacing between list indicator and elements ([#46](https://github.com/lampsitter/egui_commonmark/pull/46))

### Fixed

- Lists align text when wrapping instead of wrapping at the beginning of the next
  line ([#46](https://github.com/lampsitter/egui_commonmark/pull/46))
- Code blocks won't insert a newline when in lists
- In certain scenarios there was no newline after lists
- Copy button for code blocks show the correct cursor again on hover (regression
  after egui 0.27)

## 0.14.0 - 2024-03-26

### Added

- `AlertBundle::from_alerts`
- `AlertBundle::into_alerts`

### Changed

- Update to egui 0.27 ([#37](https://github.com/lampsitter/egui_commonmark/pull/37) by [@emilk](https://github.com/emilk))
- `CommonMarkViewer::show` returns `InnerResponse<()>`
([#36](https://github.com/lampsitter/egui_commonmark/pull/36) by [@ElhamAryanpur](https://github.com/ElhamAryanpur))

### Fixed

- A single table cell split into multiple cells ([#35](https://github.com/lampsitter/egui_commonmark/pull/35))

## 0.13.0 - 2024-02-20

### Added

- Alerts ([#32](https://github.com/lampsitter/egui_commonmark/pull/32))

> [!TIP]
> Alerts like this can be used

### Changed

- Prettier blockquotes

    Before two simple horizontal lines were rendered. Now it's a single horizontal
    line in front of the elements.

- Upgraded to pulldown-cmark 0.10

### Fixed

- Ordered lists remember their number when mixing bullet and ordered lists

## 0.12.1 - 2024-02-12

### Fixed

- Build failure with 1.72


## 0.12.0 - 2024-02-05

### Changed

- Update to egui 0.26

### Fixed

- Missing space after tables


## 0.11.0 - 2024-01-08

### Changed

- Update to egui 0.25 ([#27](https://github.com/lampsitter/egui_commonmark/pull/27) by [@emilk](https://github.com/emilk))


## 0.10.2 - 2023-12-13

### Added

- Option to change default implicit uri scheme [#24](https://github.com/lampsitter/egui_commonmark/pull/24)

## 0.10.1 - 2023-12-03

### Changed

- Code block has borders.

### Fixed

- Make code blocks non-editable ([#22](https://github.com/lampsitter/egui_commonmark/pull/22) by [@emilk](https://github.com/emilk)).


## 0.10.0 - 2023-11-23

### Changed

- Update to egui 0.24

## 0.9.2 - 2023-11-07

### Fixed

- Header sizing issues ([#20](https://github.com/lampsitter/egui_commonmark/pull/20) by [@abey79](https://github.com/abey79)).

## 0.9.1 - 2023-10-24

### Fixed

- Missing space after heading when preceded by an image
- Missing space after separator

## 0.9.0 - 2023-10-14

### Added

- Copy text button in code blocks

## 0.8.0 - 2023-09-28

### Added

- Primitive syntax highlighting by default
- Code blocks now use the syntax highlighting theme's caret and selection colors while using the
`better_syntax_highlighting` feature.
- Image loading errors are shown ([#8](https://github.com/lampsitter/egui_commonmark/pull/8) by [@emilk](https://github.com/emilk)).
- `CommonMarkCache` implements `Debug` ([#7](https://github.com/lampsitter/egui_commonmark/pull/7) by [@ChristopherPerry6060](https://github.com/ChristopherPerry6060)).
- `CommonMarkCache::add_syntax_themes_from_folder`
- `CommonMarkCache::add_syntax_theme_from_bytes`
- `CommonMarkViewer::explicit_image_uri_scheme`

### Fixed

- Links of the type ``[`fancy` _link_](..)`` is rendered correctly.

### Changed

- Update to egui 0.23
- Image formats are no longer implicitly enabled.
- Use new image API from egui ([#11](https://github.com/lampsitter/egui_commonmark/pull/11) by [@jprochazk](https://github.com/jprochazk)).
- Feature `syntax_highlighting` has been renamed to `better_syntax_highlighting`.

### Removed

- `CommonMarkCache::reload_images`
- Removed trimming of svg's transparency. The function has been removed from resvg.

## 0.7.4 - 2023-07-08

### Changed

- Better looking checkboxes

## 0.7.3 - 2023-05-24

### Added

- Support for egui 0.22. This release can also still be used with 0.21.
An explicit dependency update might be needed to use egui 0.22: `cargo update -p egui_commonmark`

## 0.7.2 - 2023-04-22

### Added

- `CommonMarkCache::clear_scrollable_with_id` to clear the cache for only a single scrollable viewer.

### Fixed

- Removed added spacing between elements in `show_scrollable`

## 0.7.1 - 2023-04-21

### Added

- Only render visible elements within a ScrollArea with `show_scrollable`
 ([#4](https://github.com/lampsitter/egui_commonmark/pull/4) by [@localcc](https://github.com/localcc)).

## 0.7.0 - 2023-02-09

### Changed

- Upgraded egui to 0.21

## 0.6.0 - 2022-12-08

### Changed

- Upgraded egui to 0.20

## 0.5.0 - 2022-11-29

### Changed

- Default dark syntax highlighting theme has been changed from base16-mocha.dark
  to base16-ocean.dark.

### Fixed

- Render text in svg images.
- Fixed erroneous newline after images.
- Fixed missing newline after lists and quotes.

## 0.4.0 - 2022-08-25

### Changed

- Upgraded egui to 0.19.

### Fixed

- Display indented code blocks in a single code block ([#1](https://github.com/lampsitter/egui_commonmark/pull/1) by [@lazytanuki](https://github.com/lazytanuki)).

## 0.3.0 - 2022-08-13

### Added

- Automatic light/dark theme in code blocks.
- Copyable code blocks.

### Changed

- Deprecated `syntax_theme` in favour of `syntax_theme_dark` and
  `syntax_theme_light`.

### Fixed

- No longer panic upon unknown syntax theme.
- Fixed incorrect line endings within headings.

