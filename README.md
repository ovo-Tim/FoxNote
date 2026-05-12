# FoxNote
A modern block-based note app focus on simplicity and flexibility.

## Key Features
### Simple
We use simple toml format to store block-based notes, no mystic binary. Readable to both you and your agents.

Example:
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

[[content]]
type = "canvas"
path = "./draw.svg"
```
We are not adding any AI features inside the app. But this design allows you to simply open your favorite agent framework and say "Organize the notes for me".

For synchronization, we simply use git, no fancy proprietary protocol, no cloud service subscription. Any devices with git installed can be used to back up your notes.

### Modern
We use modern [typst](https://github.com/typst/typst) as our primary language, with latex support(through [mitex](https://github.com/mitex-rs/mitex)).

You can also easily import traditional markdown code with latex, thanks to our markdown converter.

### Plugin System(WIP)
You can easily develop your custom block types. Add anything you like and have fun!