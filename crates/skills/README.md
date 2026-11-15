# skills

Agent Skills discovery and progressive disclosure for Brainstone.

`Catalog::load` discovers direct child directories under a supplied `skills/`
root when they contain an exact `SKILL.md` file. It advertises only names and
descriptions through the `Skill` tool definition. Calling `Skill` returns the
complete selected file and a bounded resource inventory. `SkillResource` reads
one requested UTF-8 resource without allowing paths to escape the skill root.

`SkillAdmission` expands explicit `$skill-name` mentions before handing the
prompt to the durable host. The original request remains in the accepted text,
and repeated mentions inject the selected instructions only once per prompt.

See the [operations reference](../../docs/operations.md#capabilities) for the supported file format,
runtime behavior, limits, and operator setup.
