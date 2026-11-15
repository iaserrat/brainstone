# media

Images a tool or surface hands the model, files a tool hands a reply, the name
a reply uses for either, and `CreateFile`, the tool that lets a model write a
file for the reply.
Shared by every producer of model images, so a picture an MCP server returns
is held to exactly the rules a user-supplied attachment is.

## Model input

`decode(bytes, expected)` turns untrusted bytes into an `agent::Image`:

- `media_type` accepts `image/png`, `image/jpeg`, and `image/webp` only.
- The sniffed raster format must match the declared one.
- Bytes past `MAX_IMAGE_BYTES` (5 MiB, the strictest provider's per-image
  limit) are refused before decoding; the decode buffer is bounded at 128 MiB,
  so a decompression bomb fails closed.
- A picture past the model's 1,568 px long edge is downscaled to it and
  re-encoded as PNG. One within it keeps its original bytes.

`MediaError` (`Unsupported`, `Invalid`, `TooLarge`) is a branch, not a
message: each caller maps it to its own stable code (`surface` to its
attachment skip codes, `mcp` to its omission notes).

`png(image)` re-encodes a stored image as PNG for a destination that does not
accept its format.

## References

`reference(image)` is `image:` followed by the first 64 bits of the SHA-256 of
the image's bytes, as 16 lowercase hex digits. It is a pure function of the
bytes, so the same image has the same name in every build, before and after a
deploy, and the name lives only as reply text. SHA-256 keeps a tool from
crafting a second image that answers to another's name.

`attachment(image)` is the JSON a tool's result carries for an image it
returned:

```json
{"reference": "image:039058c6f2c0cb49", "media_type": "image/png", "width": 2,
 "height": 3, "show_in_reply": "![short description](image:039058c6f2c0cb49)"}
```

`show_in_reply` is the exact Markdown that asks the consumer to attach the
image. A consumer scans the reply for a Markdown image whose destination
starts with `REFERENCE_PREFIX`, and a Markdown link whose destination starts
with `FILE_REFERENCE_PREFIX`; any other image or link stays text.

## Files

An `agent::File` is a name, a media type, and bytes that only the reply
shows. `file_reference(file)` is `file:` followed by the first 64 bits of the
SHA-256 of the name, a NUL byte, the media type, a NUL byte, and the bytes:
the same bytes under another name are another file, because the name is what
the person saves. `file_attachment(file)` is the JSON a tool's result carries
for it. A file is not shown inline, so `show_in_reply` is a Markdown link
whose text is the name, with `[`, `]`, and `\` escaped:

```json
{"reference": "file:b13912068bc2ac8f", "name": "[draft] notes.md",
 "media_type": "text/plain", "bytes": 3,
 "show_in_reply": "[\\[draft\\] notes.md](file:b13912068bc2ac8f)"}
```

`MAX_FILE_BYTES` (50 MiB) bounds a file: it is held whole in memory and in one
database row.

## CreateFile

`CreateFile` takes `name` and `content`, and optionally `encoding` (`utf-8`,
the default, or `base64` for binary bytes) and `media_type` (default
`text/plain` for text, `application/octet-stream` for base64). It returns the
file's `file_attachment` document, with the file in `ToolOutput::files` for
the runtime to store. Any agent on any provider can call it; a consumer that
attaches files offers it. Base64 is measured by the bytes it decodes to
before decoding. Each call makes one file; calling it again with the same
name, type, and content names the same file.

| Code | Meaning |
|---|---|
| `invalid_file_arguments` | The call is missing `name` or `content`, carries an unknown field, or names an encoding other than `utf-8` or `base64` |
| `invalid_file_content` | `base64` content does not decode |
| `invalid_media_type` | `media_type` is not `type/subtype` |
| `invalid_file_name` | The name is not one path segment of 1 to 255 bytes with no control character or surrounding space, or is `.` or `..` |
| `empty_file` | The content is empty |
| `file_too_large` | The content exceeds 50 MiB |
