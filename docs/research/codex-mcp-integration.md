# Codex MCP Integration: taursult

Date: 2026-03-07
Tested with: `codex-cli 0.110.0`

## Conclusion

Yes. Codex CLI has native MCP client support, including stdio servers like taursult.

The taursult server can be registered directly in Codex. A wrapper workaround is not needed for the primary use case.

## What I verified

### 1. Codex CLI supports MCP servers natively

Local Codex CLI help exposes MCP management commands:

- `codex mcp add`
- `codex mcp list`
- `codex mcp get`
- `codex mcp remove`
- `codex mcp login`
- `codex mcp logout`

Local evidence:

```bash
codex --help
codex mcp --help
codex mcp add --help
```

Official docs:

- OpenAI MCP guide: <https://platform.openai.com/docs/guides/tools-remote-mcp>
- Codex MCP docs: <https://developers.openai.com/codex/mcp>

### 2. Codex can register the taursult stdio server

I tested registration in an isolated temporary Codex home so I did not touch the real `~/.codex/config.toml`.

Test command:

```bash
tmp_home=$(mktemp -d)
HOME="$tmp_home" codex mcp add taursult -- \
  fastmcp run /home/mstie/projects/taursult/src/mcp_server/server.py
```

That succeeded.

I also verified the generated config shape with `codex mcp get taursult --json` and by reading the temporary `config.toml`.

Persisted Codex config format:

```toml
[mcp_servers.taursult]
command = "fastmcp"
args = ["run", "/home/mstie/projects/taursult/src/mcp_server/server.py"]
```

If environment variables are needed, Codex stores them under a nested env table:

```toml
[mcp_servers.taursult]
command = "fastmcp"
args = ["run", "/home/mstie/projects/taursult/src/mcp_server/server.py"]

[mcp_servers.taursult.env]
PYTHONUNBUFFERED = "1"
```

Equivalent CLI form:

```bash
codex mcp add taursult --env PYTHONUNBUFFERED=1 -- \
  fastmcp run /home/mstie/projects/taursult/src/mcp_server/server.py
```

### 3. taursult starts cleanly as a stdio MCP server

I validated the taursult server entrypoint directly:

```bash
timeout 5s fastmcp run /home/mstie/projects/taursult/src/mcp_server/server.py
```

Observed result:

- server started successfully
- FastMCP reported transport `STDIO`
- taursult logged `taursult MCP Server ready`

This confirms the server process itself is compatible with the transport Codex expects.

## Recommended setup

### Global installation for this machine

Add taursult once to the Codex config:

```bash
codex mcp add taursult -- \
  fastmcp run /home/mstie/projects/taursult/src/mcp_server/server.py
```

Verify:

```bash
codex mcp list
codex mcp get taursult --json
```

Expected config location:

- `~/.codex/config.toml`

Expected persisted entry:

```toml
[mcp_servers.taursult]
command = "fastmcp"
args = ["run", "/home/mstie/projects/taursult/src/mcp_server/server.py"]
```

### If taursult needs environment variables later

Add them at registration time:

```bash
codex mcp add taursult \
  --env OPENAI_API_KEY=$OPENAI_API_KEY \
  --env GOOGLE_API_KEY=$GOOGLE_API_KEY \
  -- fastmcp run /home/mstie/projects/taursult/src/mcp_server/server.py
```

Or edit `~/.codex/config.toml` directly:

```toml
[mcp_servers.taursult]
command = "fastmcp"
args = ["run", "/home/mstie/projects/taursult/src/mcp_server/server.py"]

[mcp_servers.taursult.env]
OPENAI_API_KEY = "..."
GOOGLE_API_KEY = "..."
```

## Practical notes

### Global vs project scope

`codex mcp add` reports `Added global MCP server ...`, and the persisted entry is written to `~/.codex/config.toml`.

Inference: the standard supported workflow today is global Codex MCP configuration, not a taurhaus-only project-local registration flow.

### Authentication column in `codex mcp list`

For the taursult stdio server, `codex mcp list` showed `Auth: Unsupported`.

That does not indicate a failure. It means there is no HTTP bearer-auth flow attached to this stdio server registration.

### End-to-end agent tool call

I did not run a live Codex session and invoke taursult tools end-to-end from inside an authenticated agent conversation.

What I did verify instead:

- Codex CLI has native MCP support
- Codex accepts the taursult registration
- Codex persists the expected config
- the taursult server starts cleanly as a stdio MCP server

That is enough to conclude the integration path is viable.

## If native MCP were unavailable

This is not the current situation, but the fallback options would be:

1. wrap a narrow subset of taursult operations as normal shell commands Codex can run
2. expose taursult through a small HTTP service and call it via scripts
3. keep image generation in Claude-only flows and pass artifacts back into Codex tasks

Because Codex already supports MCP, these workarounds are unnecessary unless team policy wants tighter tool exposure than full MCP registration.

## Recommendation

Proceed with native Codex MCP registration.

Suggested machine-level command:

```bash
codex mcp add taursult -- \
  fastmcp run /home/mstie/projects/taursult/src/mcp_server/server.py
```

Then validate locally with:

```bash
codex mcp list
codex mcp get taursult --json
```

If taursult depends on provider API keys, add them explicitly through `--env` or the `[mcp_servers.taursult.env]` table.
