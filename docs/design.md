# Tech Stack
- Rust for backend and core logic
- Tauri+Vue3+Vuetify for frontend.(pnpm)

# Key Designs
## Syncing
FoxNote leverages Git for syncing notes across devices. But of course we won't ask users to do git add, git commit by themselves. All things will be wrapped perfectly under the UI.

Benefits:
- Any devices with Git installed can be used to sync notes. If you choose to publish your nodes to a public git service like GitHub, you can even share your nodes with others easily.
- You can trace full editing history.

If there is conflict, for now we only ask user to choose A or B instead of the user to address themselves.

## Format
Just like many other modern note-taking apps, we use blocks as the basic unit of notes We mainly uses typst block, Which means this block is just simple typst codes. But we still allow you to develop your own block types. To do so, Each node actually has a toml config file to store the note content. Just like this example below

``` toml
title = "My Note"
date = "2023-01-01"
type = "notes"

[[content]]
type = "typst"
content = """
= Hello World
$E=mc^2$
"""

# If the content is too long, we can also save it in a separate file to avoid this toml file being too long.
[[content]]
type = "typst"
path = "./typst.typ"

[[content]]
type = "canvas"
path = "./draw.svg"

[[content]]
type = "custom-type"
```

### Define Custom Block Types
We currently allow you to write your own TypeScript plugins to add your custom blocks. User's plugin needs to define these things:
- How to render: Vue3 component.
- How to edit: Vue3 component. (For example, a canvas block maybe just render that as SVG while previewing, but when the user wants to edit it, you need to provide a UI to do so.)

And we will provide you APIs to read the contents you created with this plugin, and save both text and binary content.(Automatically choose to write to the TOML file or save to a new file. The plugin won't be able to know it.)

## Organize
You have basically two ways to organize your notes:
1. You can organize them just as you do in a file system. You can create deep folders and put notes in them.
2. You can organize them by tags. You can add tags to your notes, and then filter them by tags.

### Tag Management
We allow nested tags like `work/project/tasks`. And to efficiently insert and read those tags, we will need a sqlite database. But these data will only be exchanged in text form(TOML). Each time we upload these information to our git repo, we export that information to text, and each time we need to sync, we convert that information from text back to database. So that we can solve conflicts with git.
