# skills

Discovers Agent Skills and loads their instructions and resources on demand.

`Catalog::load` finds direct child directories under the supplied `skills/`
root that contain `SKILL.md`. The `Skill` tool definition advertises their
names and descriptions. Calling `Skill` returns the
complete selected file and a bounded resource inventory. `SkillResource` reads
one requested UTF-8 resource without allowing paths to escape the skill root.

`SkillAdmission` expands explicit `$skill-name` mentions before handing the
prompt to the durable host. The original request remains in the accepted text,
and repeated mentions inject the selected instructions only once per prompt.

See the [operations reference](../../docs/operations.md#capabilities) for the supported file format,
runtime behavior, limits, and operator setup.
