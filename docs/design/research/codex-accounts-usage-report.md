# Codex CLI 0.149.0: accounts and subscription usage

Research date: 2026-08-27. Installed command: `codex-cli 0.149.0` at `~/.nvm/versions/node/v24.14.1/bin/codex`. The repository and all user authentication/configuration directories were treated as read-only. No token, email, user ID, or account ID is included in this report.

## Facts

### 1. Authentication storage and account identity

The current installation is logged in with ChatGPT. `codex login status` returned `Logged in using ChatGPT`.

With the default file credential store, authentication lives at `$CODEX_HOME/auth.json`; for this process, with `CODEX_HOME` unset, that resolves to `~/.codex/auth.json`. The current file is owned by the user and has mode `0600`.

The current file has this shape. These are key names and value types only:

```text
auth.json
├── OPENAI_API_KEY: null
├── auth_mode: string                         # current value: chatgpt
├── last_refresh: RFC 3339 timestamp string
└── tokens: object
    ├── access_token: string                  # JWT
    ├── account_id: string
    ├── id_token: string                      # JWT
    └── refresh_token: string                 # opaque refresh credential
```

The source's complete `AuthDotJson` type also permits the optional keys `agent_identity`, `personal_access_token`, and `bedrock_api_key`. For API-key login it uses the serialized key name `OPENAI_API_KEY`. Codex 0.149.0 defines auth modes `apikey`, `chatgpt`, `chatgptAuthTokens` (external, in-memory tokens), `headers`, `agentIdentity`, `personalAccessToken`, and `bedrockApiKey`. The current account uses managed `chatgpt` auth, not an external-token mode.

There is no separate top-level token-expiry key. Codex reads the standard `exp` claim from the access-token JWT. In the inspected account, the access token expires at `2026-09-03T08:30:20Z`; `last_refresh` is `2026-08-24T08:30:20.731136167Z`. The ID-token JWT had an `exp` of `2026-08-24T09:30:20Z`. The refresh credential's payload was not JSON-decodable, so it should be treated as opaque rather than assumed to be a JWT.

The installed parser can derive these fields from ID-token claims:

- `email`, from the top-level claim or `https://api.openai.com/profile.email`.
- `chatgpt_plan_type`, `chatgpt_user_id`, `chatgpt_account_id`, and the FedRAMP flag from `https://api.openai.com/auth`.
- The access-token JWT also carried the profile/auth claim namespaces. Only their key names were inspected.

The currently selected ChatGPT workspace/account used for backend routing is `tokens.account_id`; Codex sends it as `ChatGPT-Account-ID`. Email is a display label and plan type is an entitlement/display value, not an account identifier. A live app-server `account/read` returned a sanitized account object equivalent to:

```json
{"account":{"type":"chatgpt","email":"<redacted>","planType":"pro"},"requiresOpenaiAuth":true}
```

`account/read` does not expose `account_id`. A coordinator that uses app-server does not need that ID to make usage requests because Codex applies it internally.

The default packaged setting is `cli_auth_credentials_store = "file"`. Source also defines `keyring`, `auto`, and `ephemeral`. The keyring service name is `Codex Auth`, and its storage key is derived from the canonical `CODEX_HOME`, so distinct homes also have distinct keyring slots.

### 2. Selecting a home and running multiple accounts

The principal per-process selector is `CODEX_HOME`:

- A non-empty `CODEX_HOME` must already exist and be a directory. Codex canonicalizes it.
- If it is empty or unset, Codex uses the OS home directory plus `.codex`.
- The resolver does not consult XDG variables.
- `CODEX_SQLITE_HOME` is a separate override for SQLite state; it does not select authentication or configuration.
- `-p`/`--profile` layers a named configuration file from the active home; it does not select another authentication file.

A safe isolated probe with a fresh temporary directory produced `Not logged in`, while the default home produced `Logged in using ChatGPT`. The temporary probe created no files. A live app-server `initialize` response reported `codexHome: ~/.codex` for the default process.

Therefore two accounts can be kept in two existing directories and selected per process, for example:

```sh
CODEX_HOME=/path/to/account-a codex
CODEX_HOME=/path/to/account-b codex app-server --listen stdio://
```

Each home scopes `auth.json`, configuration, state, and the default credential-store identity. The verified design for a coordinator is one Codex/app-server process per home. The installed public app-server schema has no exported `account/sessions/*` method for switching multiple persisted accounts inside one home.

### 3. Subscription usage endpoint and raw response

For the normal ChatGPT backend base URL, Codex 0.149.0 requests:

```http
GET https://chatgpt.com/backend-api/wham/usage
Authorization: Bearer <stored access_token>
ChatGPT-Account-ID: <stored account_id>
User-Agent: <Codex user agent>
```

It conditionally adds `X-OpenAI-Fedramp: true` for FedRAMP accounts. The rate-limit client does not add a beta header or an API-version header. The live direct GET succeeded with the first three headers above; no cookie was needed for this account. Codex has an alternate `/api/codex/usage` path style for a specially configured non-`backend-api` base URL, but that is not the path selected by the default installed configuration.

`account/rateLimits/read` also starts a concurrent GET to:

```http
GET https://chatgpt.com/backend-api/wham/rate-limit-reset-credits
```

That request uses the same backend authentication/header provider. It supplies detailed earned reset-credit rows. If it fails or exceeds its five-second timeout, app-server falls back to the summary included in the usage response.

A live response from `/wham/usage` was captured and redacted during the request pipeline, so no raw secret-bearing response was written to disk. It is saved with mode `0600` at:

`/tmp/claude-1000/-home-mstie-projects-taurhaus/f3286b16-ffc7-4d16-915d-046705823a3d/scratchpad/codex-usage-response.json`

The response's top-level keys were:

```text
user_id, account_id, email, plan_type,
rate_limit, code_review_rate_limit, additional_rate_limits,
credits, spend_control, rate_limit_reached_type, promo,
rate_limit_reset_credits
```

The raw response shape observed, with optional/null branches noted from source and schema, is:

```text
rate_limit:
  allowed: boolean
  limit_reached: boolean
  primary_window | secondary_window | null:
    used_percent: integer
    limit_window_seconds: integer
    reset_after_seconds: integer
    reset_at: integer Unix seconds
additional_rate_limits[]:
  limit_name: string
  metered_feature: string
  rate_limit: same shape as above
credits:
  has_credits: boolean
  unlimited: boolean
  overage_limit_reached: boolean
  balance: string or null
  approx_local_messages: array
  approx_cloud_messages: array
spend_control:
  reached: boolean
  individual_limit: object or null
rate_limit_reset_credits:
  available_count: integer
  applicable_available_count: integer
```

When `individual_limit` exists, the installed model/parser includes limit/used/remaining data and reset time; app-server exposes `limit`, `used`, `remainingPercent`, and `resetsAt`.

The captured, time-specific bucket values were:

| Limit ID / label | Window | Used | Duration | Reset |
|---|---:|---:|---:|---:|
| `codex` | primary | 50% | 604800 s (weekly) | Unix `1788283433` |
| `codex_bengalfox` / `GPT-5.3-Codex-Spark` | primary | 0% | 18000 s (5 hours) | Unix `1787860379` |
| `codex_bengalfox` / `GPT-5.3-Codex-Spark` | secondary | 0% | 604800 s (weekly) | Unix `1788447179` |

The base `codex` secondary window was null. Credits were present structurally but reported `has_credits: false`, `unlimited: false`, and balance `"0"`; spend control was not reached and had no individual limit. The response reported one available reset credit and zero applicable available credits. `code_review_rate_limit` was present but null, and the installed app-server mapper did not expose it in the rate-limit map.

The family set is dynamic. `additional_rate_limits[].metered_feature` becomes an app-server limit ID and `limit_name` becomes its label. The live family name above is evidence for this account at this time, not a fixed enumeration. A coordinator must not assume that primary always means five-hour or secondary always means weekly; it should use the duration and limit ID.

### 4. App-server request, response, and notifications

The installed JSON-RPC request is `account/rateLimits/read`; there is no `/read` suffix after `rateLimits` beyond that method name and no subscription request.

```json
{"method":"account/rateLimits/read","id":2}
```

`params` may be omitted/null. The response result has this installed schema:

```text
rateLimits: RateLimitSnapshot                         # required legacy/default bucket
rateLimitsByLimitId: map<string, RateLimitSnapshot> | null
rateLimitResetCredits: RateLimitResetCreditsSummary | null

RateLimitSnapshot:
  limitId: string | null
  limitName: string | null
  primary: RateLimitWindow | null
  secondary: RateLimitWindow | null
  credits: CreditsSnapshot | null
  individualLimit: SpendControlLimitSnapshot | null
  spendControlReached: boolean | null
  planType: PlanType | null
  rateLimitReachedType: enum | null

RateLimitWindow:
  usedPercent: integer
  windowDurationMins: integer | null
  resetsAt: integer Unix seconds | null

CreditsSnapshot:
  hasCredits: boolean
  unlimited: boolean
  balance: string | null

SpendControlLimitSnapshot:
  limit: string
  used: string
  remainingPercent: integer
  resetsAt: integer Unix seconds
```

The installed `rateLimitReachedType` values are `rate_limit_reached`, workspace-owner/member credit-depleted variants, workspace-owner/member usage-limit-reached variants, or null. The plan enum includes free/go/plus/pro/business/team/enterprise/education variants and `unknown`; clients should preserve unknown values rather than use the plan as identity.

`rateLimitResetCredits` contains `availableCount` plus optional credit rows. A credit row has an opaque `id`, `resetType`, `status`, `grantedAt`, optional `expiresAt`, optional `title`, and optional `description`. The live app-server response contained one redacted available `codexRateLimits` reset credit.

The notification is:

```json
{
  "method":"account/rateLimits/updated",
  "params":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":50}}}
}
```

That example illustrates the wire shape, not a complete live notification. The installed schema explicitly describes this as a sparse rolling update. The client should merge non-null values into its last full read, keyed by `limitId`, or refetch. App-server notifications are on by default after `initialize`; `initialize.params.capabilities.optOutNotificationMethods` can suppress selected methods. There is no separate subscribe RPC.

The initial account query is:

```json
{"method":"account/read","id":1,"params":{"refreshToken":false}}
```

Setting `refreshToken: true` proactively refreshes managed ChatGPT OAuth. It is ignored for externally managed tokens. These app-server methods are also documented by OpenAI's [app-server documentation](https://learn.chatgpt.com/docs/app-server).

### 5. Response-header and streaming rate-limit updates

Codex also obtains rolling limit snapshots from model-response metadata. For HTTP Responses/SSE it parses these custom header families, where `{limit}` is the normalized limit ID:

```text
x-{limit}-primary-used-percent
x-{limit}-primary-window-minutes
x-{limit}-primary-reset-at
x-{limit}-secondary-used-percent
x-{limit}-secondary-window-minutes
x-{limit}-secondary-reset-at
x-{limit}-limit-name
```

The default family is `x-codex-*`. Global fields include:

```text
x-codex-credits-has-credits
x-codex-credits-unlimited
x-codex-credits-balance
x-codex-rate-limit-reached-type
x-codex-active-limit                 # on 429
x-codex-promo-message
```

For Responses WebSocket traffic, the installed client parses an in-stream event with `type: "codex.rate_limits"`, plan type, primary/secondary window objects, credits, and a metered limit name/ID. App-server converts these internal events to `account/rateLimits/updated`.

These are distinct from the standard API-platform request/token headers such as `x-ratelimit-limit-requests`, `x-ratelimit-remaining-tokens`, and their reset counterparts. Codex 0.149.0's subscription-status mapper parses the custom `x-codex-*` families, not the standard RPM/TPM family.

### 6. Refresh and 401 behavior

There are three distinct cases:

1. A non-Codex direct GET with the stored access token performs no refresh itself. The successful live `/wham/usage` GET left the auth file's modification time and digest unchanged.
2. `account/rateLimits/read` first asks the managed auth manager for credentials. That can proactively refresh before the GET if the access JWT expires within five minutes. If expiration cannot be read, the fallback refresh threshold is eight days since `last_refresh`.
3. In the installed source, the usage endpoint's HTTP error is mapped directly to an app-server error. The `account/rateLimits/read` path has no reactive refresh-and-retry wrapper specifically for a `/wham/usage` 401. By contrast, normal model-request traffic has an unauthorized-recovery path: it first reloads auth storage if another process may have refreshed the same account, then attempts OAuth refresh and retries.

Managed OAuth refresh uses:

```http
POST https://auth.openai.com/oauth/token
Content-Type: application/json

body keys: client_id, grant_type, refresh_token
```

On a successful managed refresh, file-backed storage rewrites `auth.json` and advances `last_refresh`. The response type makes `id_token`, `access_token`, and `refresh_token` optional. Codex replaces each token only if a new value is returned; therefore a returned refresh token is stored (rotation), while an omitted/null refresh token leaves the existing one in place. Known revoked/reused/expired refresh-token responses and `invalid_grant` are treated as permanent refresh failures requiring login.

Externally managed `chatgptAuthTokens` use a different app-server callback: Codex sends the client request `account/chatgptAuthTokens/refresh` with `reason: "unauthorized"` and `previousAccountId`; the host returns `accessToken`, `chatgptAccountId`, and optional `chatgptPlanType`. Those tokens are in memory, not written to `auth.json`.

A separate process can make the subscription GET with the stored access token, as the live capture proves, but it cannot cause Codex's refresh machinery to run. Reading and replaying credentials also expands the secret-handling surface. App-server is the safer boundary for a coordinator.

### 7. TUI, status line, and hooks

`/status` is a real consumer of `account/rateLimits/read` for ChatGPT-authenticated sessions. The renderer recognizes approximately 300-minute windows as `5h limit`, 10080-minute windows as `Weekly limit`, and also has daily/monthly/annual labels. It renders percent left as `100 - usedPercent`, a progress bar, reset time, credits, and individual spend control when present.

The configurable TUI status line has `five-hour-limit` and `weekly-limit` items, so those values can be displayed continuously if configured. While turns run, rolling rate-limit events update TUI state and can emit 75%, 90%, and 95% usage warnings that direct the user to `/status`.

This is not a robust machine interface:

- `/status` performs the explicit read; the inspected prefetch poller is currently a no-op rather than a periodic background usage poll.
- `--no-alt-screen` preserves terminal content, but the result is still human-oriented rendering.
- The installed hook event enum contains tool, compaction, session, user-prompt, subagent, and stop events, but no rate-limit event.
- The legacy `notify` payload is only an `agent-turn-complete` payload; it has no usage/credit fields.

For a coordinator, `account/rateLimits/read` plus `account/rateLimits/updated` is the verified structured alternative to tailing the TUI.

### 8. API-key authentication

For API-key auth, `account/read` returns an account of `type: "apiKey"`. The installed `account/rateLimits/read` implementation rejects that mode with `chatgpt authentication required to read rate limits`, and the TUI does not prefetch ChatGPT subscription limits for it. There is therefore no equivalent 5-hour/weekly ChatGPT subscription bucket through this RPC for API-key auth.

API requests instead have normal API-platform request/token limits and billing; ChatGPT subscriptions and API billing are separate, as described in OpenAI's [billing guidance](https://help.openai.com/en/articles/8156019). The standard API response headers are documented in the [API reference](https://platform.openai.com/docs/api-reference/backward-compatibility), but this Codex subscription parser does not turn them into the `/status` five-hour/weekly display.

## How verified

- Ran `command -v codex`, `codex --version`, `codex --help`, `codex app-server --help`, `codex login --help`, and `codex login status` against the installed executable.
- Resolved the npm launcher and inspected `~/.nvm/versions/node/v24.14.1/lib/node_modules/@openai/codex`. Its package version is `0.149.0`; the launcher selects the vendored `@openai/codex-linux-x64` binary. The native binary is a stripped, static PIE ELF of 258322048 bytes with SHA-256 `bbc3341e44c9ead340ed9570c17be936e37870f570751a941699ffd04d672827`.
- Inspected only key names, types, permissions, timestamps, and safe JWT claim names/expiry values from the current auth file. No token or identifying claim value was printed or persisted.
- Downloaded and inspected the official OpenAI Codex source at tag `rust-v0.149.0`, matching the installed version. Relevant implementations were in `codex-rs/login/src/auth/`, `codex-rs/utils/home-dir/`, `codex-rs/backend-client/`, `codex-rs/codex-api/src/rate_limits.rs`, `codex-rs/app-server/`, `codex-rs/app-server-protocol/`, and `codex-rs/tui/`.
- Used `strings` and binary-safe searches on the installed native binary to confirm endpoint, header, `CODEX_HOME`, app-server method, and refresh literals. The report relies on the matching source for control flow and types.
- Generated the installed app-server JSON Schemas with `codex app-server generate-json-schema --experimental`. Inspected `GetAccountRateLimitsResponse`, `AccountRateLimitsUpdatedNotification`, account request/response, client-request, server-notification, and ChatGPT refresh schemas.
- Started the installed app-server over stdio, initialized it with experimental API capability, and sent live `account/read` and `account/rateLimits/read` requests. All output was sanitized before being retained.
- Sent one live, GET-only `/wham/usage` request using the local access credential and selected account header. The HTTP body was piped directly into an identifier/secret redaction filter and written to the requested capture path. Validated that `user_id`, `account_id`, and `email` are redacted and that the file mode is `0600`.
- Compared auth-file modification time and digest immediately before and after the successful direct GET; both were unchanged.
- Used a fresh existing temporary directory as `CODEX_HOME` for `codex login status`; it returned `Not logged in` without creating files. No logout/login/refresh action was performed against the real home.
- Cross-checked the protocol surface against OpenAI's official app-server documentation.

## Unverified

- A live 401 from `/wham/usage` was not deliberately induced. Doing so safely would not add evidence beyond the installed control flow unless a real credential were expired/revoked. The statements above about this RPC's lack of reactive retry and normal model traffic's recovery path are source-verified, not a destructive live test.
- The current refresh token was not intentionally exercised. Source verifies replacement-if-returned behavior, but whether the OAuth service rotates it on the next real refresh is server policy and was not observed.
- Two different real ChatGPT accounts were not logged in concurrently during this read-only investigation. Per-home isolation and separate credential namespaces are source-verified, and a clean-home process was live-tested, but end-to-end concurrent dual-account login was not performed.
- `/wham/usage` is an authenticated ChatGPT backend route used by this release, not a stability-guaranteed public API contract. Future Codex/backend versions can change its URL, headers, or response fields. Re-running schema generation and live compatibility tests after upgrades would verify continued behavior.
- Only the current Pro account was sampled. Other plans/accounts may return other/null windows, limit IDs, model-family labels, credits, spend-control fields, or reset-credit behavior.
- `code_review_rate_limit` was null in the live response and is not mapped by the installed app-server rate-limit response. Its non-null schema/semantics were not established.
- No real API-key account was live-tested. The absence of ChatGPT subscription buckets for that mode is verified from the installed guard and protocol behavior.

## Recommendation for a coordinator

Use one private, already-existing `CODEX_HOME` per ChatGPT account and run one long-lived `codex app-server --listen stdio://` child per home. Treat the coordinator's home handle as the durable internal account key; show the sanitized `account/read` email and plan only as UI metadata. Do not make email or plan the unique key. Let each app-server retain and refresh its own selected `account_id` and tokens.

After `initialize`, call `account/read` with `refreshToken: false`, then `account/rateLimits/read`. Do not opt out of `account/rateLimits/updated`. Cache the full read by `limitId`; merge sparse notification fields, or refetch when merging is ambiguous. Render windows from `windowDurationMins` rather than assuming primary=five-hour and secondary=weekly. Treat `rateLimitsByLimitId` as a dynamic map so model-family limits appear automatically.

Use a modest fallback poll only for periods with no model traffic or after app-server restart; notifications are event-driven and `/status` is not backed by a periodic poller in this release. Apply backoff on errors and refetch after authentication changes. The app-server boundary is preferable to copying tokens into the desktop process because it centralizes proactive refresh, account headers, future protocol changes, and secret persistence.

If a direct GET is retained as a diagnostic fallback, keep it GET-only, never log request headers or raw bodies, redact identity fields before persistence, and expect it to fail after token expiry without self-refresh. Do not independently write `auth.json`; concurrent Codex processes already have a same-account reload path, while an external writer risks corrupting or racing credential rotation.

For API-key accounts, show API billing/rate-limit telemetry separately from ChatGPT subscription usage. Do not label standard RPM/TPM limits as the five-hour or weekly Codex subscription buckets.
