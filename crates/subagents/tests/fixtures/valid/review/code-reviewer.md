---
name: code-reviewer
description: Reviews code after implementation
tools: Read, mcp__github__*
disallowedTools:
  - mcp__github__delete
model: sonnet
maxTurns: 12
effort: high
background: true
mcpServers:
  - github
---
Review carefully.
Return only findings.
