# minimal-agent

A complete Brainstone agent directory. Pass it to `Settings::from_map`, or
run here when using `Settings::from_env`. The engine loads the files below.

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
