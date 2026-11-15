# Failure codes

Expected failures expose stable codes so tools, recovery logic, and operators do
not need to parse messages. Provider, agent, and runtime codes are
documented in their crate READMEs. The service-level and extension codes are
listed here.

| Code | Meaning |
|---|---|
| `telemetry_configuration` / `telemetry_already_installed` | Telemetry settings are invalid, or another global subscriber is already installed; startup fails without exposing setting values |
| `telemetry_trace_export_failed` / `telemetry_metric_export_failed` | Diagnostic only: remote export failed; telemetry is dropped and agent work continues |
| `telemetry_trace_shutdown_failed` / `telemetry_metric_shutdown_failed` / `telemetry_shutdown_timeout` | Diagnostic only: shutdown could not flush pending telemetry; application result is preserved |
| `telemetry_log_queue_full` | Diagnostic only: the bounded stderr queue dropped records; reported during shutdown |
| `invalid_configuration` | A required environment value is absent or malformed, or a prompt, agent, or MCP config file is invalid |
| `model_selection_failed` | The deployment's model selector (`DYNAMIC_MODEL_AND_EFFORT_COMMAND`) failed to start, timed out, or did not answer exactly one model and one effort level; the prompt was not accepted |
| `unsupported_tools` / `unsupported_images` / `unsupported_tool_images` / `unsupported_reasoning_effort` | The selected model cannot honor the request; input is preserved and rejected before network I/O |
| `invalid_request_parameter` | Operator-supplied model parameters would replace harness-owned conversation or persistence fields |
| `tool_policy_denied` / `tool_approval_denied` / `tool_approval_expired` / `tool_approval_unsupported` | This attempt did not dispatch the tool: policy denied it, an operator denied it, authorization expired, or the store cannot persist required approvals |
| `operation_busy` | Operator recovery refused an active session lease; wait for the current worker or normal lease expiry |
| `operation_or_approval_not_found` | An explicit operator lookup did not find the requested record |
| `tool_call_superseded` | Steering skipped this tool call in the current attempt and closed its result before the correction; an earlier crashed attempt may have produced effects, requiring reconciliation before retry |
| `mcp_configuration` | An MCP definition, URL, header, timeout, or referenced environment value is invalid |
| `mcp_connect` / `mcp_startup_timeout` | A required MCP server could not negotiate before startup |
| `mcp_list_tools` / `mcp_invalid_tool_name` | MCP discovery returned an unusable tool set |
| `mcp_queue_timeout` | A call waited too long for its server; no request was sent |
| `mcp_tool_timeout` / `mcp_tool_call` | A remote tool timed out or failed at the protocol layer |
| `mcp_invalid_arguments` / `mcp_result_encode` / `mcp_tool_error` | A call or completed tool result was invalid, unencodable, or marked as an error |
| `mcp_binary_omitted` | Not a failure: names the note that replaces an audio block in a result the model reads, and every binary in a result the server marked as an error |
| `mcp_image_unsupported` / `mcp_image_invalid` / `mcp_image_too_large` | Not a failure: names the note that replaces an image block the model did not receive because it is not PNG/JPEG/WebP, is not valid base64 or does not decode as its declared type, or exceeds 5 MiB or the decode bound |
| `mcp_video_unsupported` | Not a failure: names the note that replaces a `video/*` blob resource, because the reply does not carry video |
| `mcp_file_invalid` / `mcp_file_too_large` | Not a failure: names the note that replaces any other blob resource the reply cannot attach because it is not valid base64, its URI or media type names no file, or it exceeds 50 MiB |
| `mcp_input` / `mcp_unsupported_input` / `mcp_input_rounds_exceeded` | Interactive input failed, requested deprecated sampling or roots, or exceeded the round limit |
| `mcp_task_failed` / `mcp_task_cancelled` / `mcp_task_protocol` / `mcp_task_result` | A materialized task reached the named terminal or protocol failure |
| `mcp_closed` / `mcp_shutdown` | An MCP connection was already closed or could not shut down cleanly |
| `agent_definition_io` / `invalid_agent_frontmatter` / `invalid_agent_definition` | A project-agent file could not be read or parsed |
| `unsupported_agent_field` / `duplicate_agent_name` | A definition uses unsupported host behavior or repeats a name |
| `unknown_agent_tool` / `unknown_agent_mcp_server` / `agent_has_no_tools` | A definition does not resolve against the startup tool catalog |
| `unknown_agent` / `agent_denied` / `agent_depth_exceeded` | Delegation names an unavailable child, violates its allowlist, or exceeds depth 3 |
| `durable_context_required` / `agent_concurrency_exceeded` | A control call is outside durable execution or the tree already has 20 active children |
| `agent_cancelled` / `agent_lost` | A child was stopped or its durable operation could not be restored |
| `compaction_failed` | The model did not return a nonempty terminal summary without tool calls |
| `context_window_exceeded` | The provider refused a request as too long for its context window, and one turn alone was too long to summarize; the lane's next prompt recovers it |
| `schedule_not_found` | Scheduled work is absent, not visible to the caller, or no longer in the required state |
| `schedule_limit_exceeded` / `schedule_owner_limit_exceeded` | The lane (20) or the owner across lanes (50) already holds the maximum standing schedules |
| `invalid_schedule_arguments` / `schedule_requires_durability` / `invalid_schedule_result` | A scheduling call is malformed, lacks durable execution context, or cannot encode its result |
| `schedule_interval_too_short` | `repeat_every_minutes` is below the operator's `SCHEDULE_MIN_INTERVAL_MINUTES` |
| `schedule_destination_forbidden` | The surface's `Destinations` implementation refused to resolve the requested post target |
| `schedule_post_rate_exceeded` | The requester already created 10 posting schedules in the trailing hour |
| `invalid_file_arguments` / `invalid_file_content` / `invalid_file_name` / `invalid_media_type` / `empty_file` / `file_too_large` | A `CreateFile` call is malformed or names an unknown encoding, carries base64 that does not decode, names a file a person could not save as given (not one path segment of 1 to 255 bytes, a control character, surrounding spaces, `.` or `..`), names a media type that is not `type/subtype`, has empty content, or exceeds 50 MiB |
| `invalid_memory_arguments` | A memory tool call is malformed or missing required content |
| `memory_actor_required` | A user-scoped memory call ran in an operation whose prompt carries no actor |
| `invalid_skill_arguments` / `unknown_skill` | A skill tool call is malformed or names an unavailable skill |
| `invalid_skill_resource_path` / `skill_resource_io` | A requested skill resource is unsafe, missing, or unreadable |
| `skill_resource_not_text` / `skill_resource_too_large` | A resource cannot be inserted into text context safely |
| `too_many_attachments` | A prompt file past its kind's count cap (four images, four text files) was skipped |
| `attachment_too_large` | A file past its byte cap or the prompt's 10 MiB shared budget, or a decode too large to buffer, was skipped |
| `unsupported_attachment` | A file was not a PNG/JPEG/WebP image or a `text/*` file |
| `invalid_attachment` | File bytes were corrupt or contradicted their declared type |
| `file_unavailable` | A private file was absent, inaccessible, redirected, or untrusted |
| `invalid_thread_arguments` / `invalid_thread_file_arguments` | A `ReadThread` or `ReadThreadFile` call is malformed, or its `limit` is outside 1 to 200 |
| `thread_unavailable` | The calling operation has no surface thread (child agents) |
| `unknown_thread_message` | `ReadThread`'s `before` names no message of the thread |
| `file_not_in_thread` | `ReadThreadFile` names a file id that no message of the calling operation's thread carries, or one posted after the first 1,000 messages of a longer thread |
| Component code | The provider, host, or runtime stopped the service |

Component references:

- [Agent](../crates/agent/README.md)
- [Anthropic](../crates/anthropic/README.md)
- [OpenAI](../crates/openai/README.md)
- [Runtime](../crates/runtime/README.md)

Identity-bound tools: `authenticated_actor_required` rejects absent/invalid
principal facts; `private_conversation_required` rejects conversations not
private to the caller.
`execution_identity_conflict` rejects incompatible durable lane identities.
See [the execution context contract](mcp-execution-context.md).
