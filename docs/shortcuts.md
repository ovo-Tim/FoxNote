# FoxNote Shortcuts

This document lists the shortcuts currently implemented in the app.

## Global

- `Esc`
  - Blur the current focused element.
- `Cmd/Ctrl + S`
  - Commit the current note when the Notes view is active and the note has changes.

## Note tree

- `F2`
  - Start inline rename for the currently selected folder or note.
  - Works only when exactly one row is selected.
- While renaming a folder:
  - `Enter` → confirm rename
  - `Esc` → cancel rename
- While renaming a note:
  - `Enter` → confirm rename
  - `Esc` → cancel rename

## Note editor

### General editor focus

- `Cmd/Ctrl + F`
  - Open the in-page find bar for the current note.
  - Search covers the note title, tags, and visible blocks.
- In the find bar:
  - `Enter` → jump to the next match
  - `Shift + Enter` → jump to the previous match
  - `Esc` → close the in-page find bar
- `Cmd/Ctrl + Enter`
  - Finish current block/title editing, blur focus, and close editor menus.

### Title editing

- `Enter`
  - Finish title editing.
- `Esc`
  - Finish title editing.

### Tag menu

- `Enter`
  - Create the current tag query as a tag.

### Selected blocks

- `Backspace` or `Delete`
  - Delete the selected block.
- `Cmd/Ctrl + D`
  - Duplicate the selected block.
- `Cmd/Ctrl + C`
  - Copy the selected blocks.
- `Esc`
  - Clear block selection.

## Typst block editor

### Formatting

- `Cmd/Ctrl + B`
  - Wrap selection with `*...*`.
- `Cmd/Ctrl + I`
  - Wrap selection with `_..._`.
- `Cmd/Ctrl + U`
  - Wrap selection with `#underline[...]`.

### note-me boxes

- `Cmd/Ctrl + Shift + 1`
  - Insert or wrap with `#note[...]`.
- `Cmd/Ctrl + Shift + 2`
  - Insert or wrap with `#tip[...]`.
- `Cmd/Ctrl + Shift + 3`
  - Insert or wrap with `#important[...]`.
- `Cmd/Ctrl + Shift + 4`
  - Insert or wrap with `#warning[...]`.
- `Cmd/Ctrl + Shift + 5`
  - Insert or wrap with `#caution[...]`.
- `Cmd/Ctrl + Shift + 6`
  - Insert or wrap with `#todo[...]`.
