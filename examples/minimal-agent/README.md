# minimal-agent

A complete Brainstone agent directory. Point `Settings::from_map` at this
directory (or run with it as the working directory for `Settings::from_env`)
and the engine composes from it.

```text
minimal-agent/
  prompts/
    system_prompt.md   # required: valid UTF-8, non-blank; loaded once at startup
  mcp.toml             # optional: MCP tool servers (empty example)
  skills/              # optional: one Agent Skill per direct child directory
```

The engine also reads `agents/**/*.md` (child agents) if the directory exists;
this example has none. Edits to prompts, agents, and skills load on restart;
already-created sessions keep their stored snapshot.
