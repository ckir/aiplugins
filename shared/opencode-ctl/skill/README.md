# opencode-ctl skill templates

`opencode-ctl-driver.md` and `opencode-ctl-doctor.md` beside this file are the canonical,
source-of-truth templates for the two opencode-ctl-mcp skills.

They are templates, not the shipped copies: a renderer substitutes the host placeholders to emit one copy
per host, and each plugin's binary embeds its rendered copy at compile time.

The emitted copies live under each plugin's `skills/` directory and are generated. Do not edit them by
hand: your changes will be overwritten on the next emit. Edit only the two canonical files here.
