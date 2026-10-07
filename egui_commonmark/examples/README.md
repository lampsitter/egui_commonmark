# Overview of examples

## hello_world.rs

A good starting point.

## interactive.rs

An interactive example where you can play around with the markdown text at
runtime.

## book.rs

Intended to show all the different rendering features of the crate.

## macros.rs

Features compile time markdown parsing.

## show_mut.rs

How to make checkboxes interactive.

## link_hooks.rs

Allow hijacking links for doing operations within the application such as
changing a markdown page in a book without displaying the destination link.

## mixing.rs

Shows commonmark elements mixed with egui widgets. It displays the widgets with
no spaces in between as if the markdown was egui widgets.

## search.rs

Shows how to perform search in the markdown. It is recommended to turn on
optimization in Cargo.toml for the best experience.

## scroll.rs

Shows how to have large markdown documents with scrolling while maintaining
performance. It is recommended to turn on optimization in Cargo.toml for the best experience.
