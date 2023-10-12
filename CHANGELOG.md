# egui_commomnark changelog

## Unreleased


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

