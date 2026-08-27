# Gemini CLI — Multi-Account + Quota/Usage Research

**Pinned version:** `v0.57.0` (released 2026-08-25), tag object → commit **`6b0ae9a6c37aa117cc8b070d8b41c5bb4fa6d253`**
All `raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/...` URLs below resolve to that commit.
**Not installed on this host** (`which gemini` → not found; `~/.gemini` does not exist). Everything here is read from published source/docs, not from a local install.

---

## FACTS

### 1. Config home, credential files, and the per-process env var

**`GEMINI_CLI_HOME` is the env var. There is no `GEMINI_CONFIG_DIR`, and no XDG support.**

`packages/core/src/utils/paths.ts`:

```typescript
export const GEMINI_DIR = '.gemini';
export const GOOGLE_ACCOUNTS_FILENAME = 'google_accounts.json';
export const TRUSTED_FOLDERS_FILENAME = 'trustedFolders.json';

export function homedir(): string {
  const envHome = process.env['GEMINI_CLI_HOME'];
  if (envHome) {
    return envHome;
  }
  return os.homedir();
}
```

`packages/core/src/config/storage.ts` imports **that** `homedir`, not `node:os`'s:

```typescript
import {
  GEMINI_DIR, homedir, GOOGLE_ACCOUNTS_FILENAME, isSubpath,
  resolveToRealPath, normalizePath,
} from '../utils/paths.js';

static getGlobalGeminiDir(): string {
  const homeDir = homedir();
  if (!homeDir) return path.join(os.tmpdir(), GEMINI_DIR);
  return path.join(homeDir, GEMINI_DIR);
}
static getOAuthCredsPath(): string {
  return path.join(Storage.getGlobalGeminiDir(), OAUTH_FILE);      // OAUTH_FILE = 'oauth_creds.json'
}
static getGoogleAccountsPath(): string {
  return path.join(Storage.getGlobalGeminiDir(), GOOGLE_ACCOUNTS_FILENAME);
}
static getInstallationIdPath(): string {
  return path.join(Storage.getGlobalGeminiDir(), 'installation_id');
}
static getGlobalTempDir(): string {
  return path.join(Storage.getGlobalGeminiDir(), TMP_DIR_NAME);     // 'tmp'
}
getHistoryDir(): string { /* <geminiDir>/history/<projectIdentifier> */ }
getProjectTempLogsDir(): string { /* <geminiDir>/tmp/<hash>/logs */ }
```

Because **every** global path funnels through `getGlobalGeminiDir()` → `homedir()`, setting `GEMINI_CLI_HOME=/some/root` relocates the *entire* profile: `/some/root/.gemini/{settings.json, oauth_creds.json, google_accounts.json, installation_id, tmp/, history/}`.

Official confirmation — `docs/cli/enterprise.md`:

> "By default, Gemini CLI stores configuration and history in `~/.gemini`. You can use the `GEMINI_CLI_HOME` environment variable to point to a unique directory for a specific user or job. The CLI will create a `.gemini` folder inside the specified path."

**Key names inside `oauth_creds.json`** (google-auth-library `Credentials`, written verbatim as JSON): `access_token`, `refresh_token`, `id_token`, `expiry_date`, `scope`, `token_type`. Written with `mode: 0o600`:

```typescript
async function cacheCredentials(credentials: Credentials) {
  const filePath = Storage.getOAuthCredsPath();
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  const credString = JSON.stringify(credentials, null, 2);
  await fs.writeFile(filePath, credString, { mode: 0o600 });
}
```

**Other path-override env vars** (narrower, file-specific, not a config-dir switch): `GEMINI_CLI_SYSTEM_SETTINGS_PATH`, `GEMINI_CLI_TRUSTED_FOLDERS_PATH` (both read in `storage.ts`).

**Keychain caveat — but it is opt-in, not default.** `oauth2.ts`:

```typescript
function getUseEncryptedStorageFlag() {
  return process.env[FORCE_ENCRYPTED_FILE_ENV_VAR] === 'true';
}
```
with `packages/core/src/mcp/token-storage/index.ts`:
```typescript
export const FORCE_ENCRYPTED_FILE_ENV_VAR = 'GEMINI_FORCE_ENCRYPTED_FILE_STORAGE';
```
Only when that env var is literally `'true'` do credentials go to `OAuthCredentialStorage` (`HybridTokenStorage` → OS keychain, service `gemini-cli-oauth`, account `main-account`, with encrypted-file fallback). Load order:

```typescript
const pathsToTry = [
  ...(!useEncryptedStorage ? [Storage.getOAuthCredsPath()] : []),
  process.env['GOOGLE_APPLICATION_CREDENTIALS'],
].filter((p): p is string => !!p);
```

So **by default the plaintext `oauth_creds.json` is authoritative** and a coordinator can read it.

### 2. What identifies the account

`packages/core/src/utils/userAccountManager.ts` — `UserAccountManager`, persisted to `Storage.getGoogleAccountsPath()`:

```typescript
interface UserAccounts {
  active: string | null;   // the signed-in email
  old: string[];           // previously used emails
}
```
Default on missing file: `{ active: null, old: [] }`. Methods: `cacheGoogleAccount(email)` (moves previous `active` into `old`), `getCachedGoogleAccount()`, `getLifetimeGoogleAccounts()` (count), `clearCachedGoogleAccount()`. Sync variant `readAccountsSync()` exists — safe for a coordinator to poll cheaply.

The email originates from `oauth2.ts` fetching **`https://www.googleapis.com/oauth2/v2/userinfo`** with the access token, then `userAccountManager.cacheGoogleAccount(userInfo.email)`. `id_token` in `oauth_creds.json` is a JWT that also carries the email claim.

Auth mode is recorded in `settings.json` under `security.auth.selectedType` (values `oauth-personal`, `gemini-api-key`, `vertex-ai`, ...).

**Two accounts side by side: YES, via `GEMINI_CLI_HOME` per process.** Each root gets its own `oauth_creds.json` + `google_accounts.json` + `settings.json`. Launch selectively by setting the env var on the child process only. There is **no** native account-switch flag; multi-account is an open, unimplemented request (issue #3565, closed/stale; third-party tools like `aisw` and Gemini-CLI-Auth-Manager exist purely to shuffle these dirs).

Caveat: if a user sets `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true`, both profiles collapse onto the **same** keychain entry (`gemini-cli-oauth` / `main-account`) — `GEMINI_CLI_HOME` does not namespace the keychain. A coordinator should not set that var.

### 3. Quota display

Three surfaces, all fed by **one** API call:

- **`/stats`** (`packages/cli/src/ui/commands/statsCommand.ts`): subcommands `session`, `model`, `tools`. `session` and `model` call `refreshUserQuota()` and render pooled remaining / limit / reset time. `tools` shows no quota.
- **Footer** — configurable via `/footer` (aka `/statusline`) and `ui.footer.*` settings (e.g. `hideContextPercentage`, `hideModelInfo`).
- **`refreshUserQuota()`** in `packages/core/src/config/config.ts` is the single source; it populates an **in-memory** map.

**The endpoint:**

`packages/core/src/code_assist/server.ts`:
```typescript
export const CODE_ASSIST_ENDPOINT = 'https://cloudcode-pa.googleapis.com';   // override: process.env['CODE_ASSIST_ENDPOINT']
export const CODE_ASSIST_API_VERSION = 'v1internal';                          // override: process.env['CODE_ASSIST_API_VERSION']
// getMethodUrl(method) => `${endpoint}/${version}:${method}`

async retrieveUserQuota(req: RetrieveUserQuotaRequest): Promise<RetrieveUserQuotaResponse> {
  return this.requestPost<RetrieveUserQuotaResponse>('retrieveUserQuota', req);
}
```

| | |
|---|---|
| **URL** | `POST https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota` |
| **Headers** | `Content-Type: application/json` + `Authorization: Bearer <access_token>` (injected by google-auth-library `client.request()`) |
| **Body** | `{"project": "<projectId>"}` |
| **Retry** | `statusCodesToRetry: [[429,429],[499,499],[500,599]]`, 3 attempts, 100ms base delay |

Types (`packages/core/src/code_assist/types.ts`):
```typescript
export interface RetrieveUserQuotaRequest { project: string; userAgent?: string; }
export interface BucketInfo {
  remainingAmount?: string;    // omitted by the API when quota is full (issue #27363)
  remainingFraction?: number;  // 0..1
  resetTime?: string;          // RFC3339
  tokenType?: string;
  modelId?: string;
}
export interface RetrieveUserQuotaResponse { buckets?: BucketInfo[]; }
```

Sibling RPCs on the same `v1internal:` colon-method base: `loadCodeAssist`, `onboardUser`, `countTokens`, `generateContent`, `streamGenerateContent`, `fetchAdminControls`, `listExperiments`, `recordCodeAssistMetrics`, `get/setCodeAssistGlobalUserSetting`.

**projectId** (`packages/core/src/code_assist/setup.ts`): `process.env['GOOGLE_CLOUD_PROJECT'] || process.env['GOOGLE_CLOUD_PROJECT_ID']`, else from `loadCodeAssist`'s `cloudaicompanionProject` response field. `refreshUserQuota()` **returns `undefined` early if `projectId` is falsy** — so the project ID is required in-CLI:

```typescript
const codeAssistServer = getCodeAssistServer(this);
if (!codeAssistServer || !codeAssistServer.projectId) return undefined;
```

A coordinator that has never seen the project ID can obtain it itself with one `POST .../v1internal:loadCodeAssist` (body `{"metadata":{"ideType":"GEMINI_CLI","pluginType":"GEMINI"}}`) and read `cloudaicompanionProject` from the response.

**Can an external process call it with the stored token without triggering refresh?** **Yes.** A plain HTTPS POST with `Authorization: Bearer <access_token>` read from `oauth_creds.json` touches none of the CLI's auth machinery — no file is written, no refresh occurs, and the CLI is not disturbed. This is exactly what the third-party CodexBar menubar app does.

**What a 401 does.** In the CLI: 401 is **not** in `statusCodesToRetry`, and `server.ts` contains no 401-specific handling — it propagates. Refresh is handled one layer down by google-auth-library, and *when a refresh happens* the CLI **does rewrite the credential file**:

```typescript
client.on('tokens', async (tokens: Credentials) => {
  if (useEncryptedStorage) {
    await OAuthCredentialStorage.saveCredentials(tokens);
  } else {
    await cacheCredentials(tokens);   // rewrites ~/.gemini/oauth_creds.json, mode 0600
  }
  await triggerPostAuthCallbacks(tokens);
});
```

Startup validation is `getAccessToken()` → `getTokenInfo(token)`; on throw it logs `'Cached credentials are not valid:'` and falls through to interactive auth. There is no `invalid_grant`-specific branch.

For a **coordinator**, the safe posture: read `expiry_date` from `oauth_creds.json`, and if expired do **not** refresh yourself — a coordinator-issued refresh mints a new token and (because Google may rotate refresh tokens) can desync the CLI's on-disk copy. Treat expiry as "unknown, show stale-with-timestamp" and let the CLI refresh on its next run.

### 4. Local files a coordinator could tail instead of calling the API

**There is no persisted quota/usage cache.** `config.ts`:

```typescript
private modelQuotas: Map<string, { remaining: number; limit: number; resetTime?: string }> = new Map();
```
In-memory only — no `fs.writeFile`, no `Storage` call near it; cleared on `setSessionId()`. The `/usage` "cache" in issue #27363 is this map, not a file. So **tailing cannot replace the quota API.**

What *is* tailable:

- **OpenTelemetry file export** — the real option. Settings under `telemetry.*`: `enabled`, `target` (`local`|`gcp`), `otlpEndpoint`, `otlpProtocol`, `logPrompts`, `outfile`, `useCollector`, `useCliAuth`. `packages/core/src/telemetry/file-exporters.ts` (`FileSpanExporter`, `FileLogExporter`, `FileMetricExporter`) writes **one JSON object per line** (`safeJsonStringify(data, 2) + '\n'`, no array wrapper — note the `2`-space indent means records span multiple lines, so parse by object, not strictly by line).
  Metric names (`packages/core/src/telemetry/metrics.ts`): `gemini_cli.token.usage` (attributes `model`, `type` ∈ `input|output|thought|cache|tool`), `gemini_cli.api.request.count`, `gemini_cli.api.request.latency`, `gemini_cli.session.count`, `gemini_cli.tool.call.count`, `gemini_cli.tool.call.latency`, `gemini_cli.file.operation.count`, `gemini_cli.lines.changed`, `gemini_cli.agent.run.count`, `gemini_cli.agent.duration`, `gemini_cli.network_retry.count`, `gemini_cli.chat.invalid_chunk.count`, `gemini_cli.chat.content_retry.count`.
  Common attributes include `session.id`, `installation.id`, `user.email` — the last one lets a coordinator attribute a telemetry stream to an account without reading credentials at all.
- `<geminiDir>/tmp/<projectHash>/logs/` — per-project temp logs.
- `<geminiDir>/history/<projectIdentifier>` — chat history.
- `<geminiDir>/installation_id` — stable install identifier.
- `<geminiDir>/google_accounts.json` — active/old emails (cheap to poll; changes only on login/logout).

Telemetry gives **consumption** (tokens spent). Only `retrieveUserQuota` gives **remaining/limit/reset**. A coordinator wanting a quota gauge must call the API; telemetry alone can only show burn.

### 5. Auth modes and which have usage buckets

`packages/core/src/core/contentGenerator.ts`:
```typescript
export enum AuthType {
  LOGIN_WITH_GOOGLE = 'oauth-personal',
  USE_GEMINI = 'gemini-api-key',
  USE_VERTEX_AI = 'vertex-ai',
  LEGACY_CLOUD_SHELL = 'cloud-shell',
  COMPUTE_ADC = 'compute-default-credentials',
  GATEWAY = 'gateway',
}
```
Env-var detection order: `GOOGLE_GENAI_USE_GCA=true` → `LOGIN_WITH_GOOGLE`; `GOOGLE_GENAI_USE_VERTEXAI=true` → `USE_VERTEX_AI`; `GOOGLE_GEMINI_BASE_URL` present → `GATEWAY`; `GEMINI_API_KEY` present → `USE_GEMINI`; `CLOUD_SHELL=true` or `GEMINI_CLI_USE_COMPUTE_ADC=true` → `COMPUTE_ADC`. Vertex additionally requires `GOOGLE_API_KEY` **or** both `GOOGLE_CLOUD_PROJECT` and `GOOGLE_CLOUD_LOCATION`.

`packages/core/src/code_assist/codeAssist.ts` — `createCodeAssistContentGenerator` builds a `CodeAssistServer` for **only** `AuthType.LOGIN_WITH_GOOGLE` and `AuthType.COMPUTE_ADC`; anything else throws `Unsupported authType: ${authType}`.

| Auth mode | `oauth_creds.json` present | `retrieveUserQuota` buckets available |
|---|---|---|
| `oauth-personal` (LOGIN_WITH_GOOGLE) | **yes** | **yes** — the only mode a desktop coordinator should target |
| `compute-default-credentials` | no (ADC/metadata) | yes, in principle (same CodeAssistServer path) |
| `gemini-api-key` (USE_GEMINI) | no | **no** — no Code Assist server; billing/limits live in AI Studio / Cloud quotas |
| `vertex-ai` | no (ADC or API key) | **no** — Cloud Monitoring quota metrics instead |
| `cloud-shell` (legacy), `gateway` | no | no |

---

## HOW VERIFIED (URLs + commit)

All source read at tag **`v0.57.0`** = commit `6b0ae9a6c37aa117cc8b070d8b41c5bb4fa6d253`.

| Claim | Source |
|---|---|
| Pinned version/commit | `https://api.github.com/repos/google-gemini/gemini-cli/releases/latest` → `v0.57.0`, published `2026-08-25T18:37:14Z`; `https://api.github.com/repos/google-gemini/gemini-cli/git/ref/tags/v0.57.0` → sha `6b0ae9a6c37aa117cc8b070d8b41c5bb4fa6d253` |
| `GEMINI_CLI_HOME`, `GEMINI_DIR`, `GOOGLE_ACCOUNTS_FILENAME` | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/utils/paths.ts` |
| All global paths route through `homedir()`; `OAUTH_FILE`; installation_id/tmp/history | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/config/storage.ts` |
| `GEMINI_CLI_HOME` documented behavior | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/docs/cli/enterprise.md` |
| `UserAccounts {active, old}`, read/write impl | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/utils/userAccountManager.ts` |
| userinfo endpoint, `cacheCredentials` mode 0600, `tokens` event rewrite, `getUseEncryptedStorageFlag`, load order | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/code_assist/oauth2.ts` |
| `FORCE_ENCRYPTED_FILE_ENV_VAR = 'GEMINI_FORCE_ENCRYPTED_FILE_STORAGE'` | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/mcp/token-storage/index.ts` |
| Keychain service/account names, legacy-file migration | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/code_assist/oauth-credential-storage.ts`; `.../mcp/token-storage/hybrid-token-storage.ts` |
| Endpoint constants, method URL shape, `retrieveUserQuota`, `statusCodesToRetry` | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/code_assist/server.ts` |
| `RetrieveUserQuotaRequest/Response`, `BucketInfo`, `LoadCodeAssist*`, `ClientMetadata` | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/code_assist/types.ts` |
| `modelQuotas` in-memory only, `refreshUserQuota()` early-return on missing projectId | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/config/config.ts` |
| Only LOGIN_WITH_GOOGLE + COMPUTE_ADC build a CodeAssistServer | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/code_assist/codeAssist.ts` |
| `AuthType` enum + env detection order | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/core/contentGenerator.ts` |
| projectId resolution, `cloudaicompanionProject`, `setupUser` return | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/code_assist/setup.ts` |
| `/stats` subcommands + which render quota | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/cli/src/ui/commands/statsCommand.ts` |
| Telemetry settings keys | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/docs/cli/telemetry.md` |
| One-JSON-object-per-record file export | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/telemetry/file-exporters.ts` |
| Metric names + token attributes | `https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.57.0/packages/core/src/telemetry/metrics.ts` |
| `remainingAmount` omitted at 100% quota | Issue #27363 `https://github.com/google-gemini/gemini-cli/issues/27363` |
| Multi-account is unimplemented | Issue #3565 `https://github.com/google-gemini/gemini-cli/issues/3565` (closed/stale, p2) |
| Third-party confirmation of Bearer-token quota call + refresh URL | `https://github.com/steipete/CodexBar/blob/main/docs/gemini.md` (not this repo; corroboration only) |

Local check: `which gemini` → not found; `ls ~/.gemini` → does not exist. No files were read from or written to `~/.gemini`, `~/.codex`, or `~/.claude*`. No git write commands were run. No token values are reproduced anywhere in this report.

---

## UNVERIFIED

1. **`retrieveUserQuota` with an empty/absent `project`.** CodexBar's docs claim it sends `{}` when the project is unknown, but `RetrieveUserQuotaRequest.project` is typed as required `string` and the CLI never calls it without one. *Verify:* one live POST with `{}` against a real token, compare to `{"project":"..."}`.
2. **google-auth-library's exact 401 behavior.** I verified the CLI adds no 401 handling and that a refresh rewrites the creds file. I did **not** read google-auth-library's source to confirm whether `client.request()` retries a 401 once with a forced refresh, or only refreshes proactively on `expiry_date`. *Verify:* read `google-auth-library` `OAuth2Client.requestAsync` at the version in the CLI's lockfile.
3. **Whether Google rotates the refresh token on refresh for this client.** My "don't refresh from the coordinator" advice assumes it might. *Verify:* refresh twice against a scratch account and diff the `refresh_token` field.
4. **`/usage` as a distinct slash command.** Issue #27363 refers to `/usage`, but `packages/cli/src/ui/commands/usageCommand.ts` 404s at v0.57.0. It may be named differently or be a `/stats` alias. *Verify:* list `packages/cli/src/ui/commands/` at the tag.
5. **Exact footer quota setting key.** `/footer` and `ui.footer.{hideContextPercentage,hideModelInfo}` are attested via issues/docs, but I did not read `settingsSchema.ts` for a quota-specific key. *Verify:* read `packages/cli/src/config/settingsSchema.ts`.
6. **Windows keychain path/behavior** under `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true`, and the encrypted-file fallback's on-disk location. `KeychainTokenStorage` not read.
7. **Whether COMPUTE_ADC actually returns quota buckets** in practice — it shares the CodeAssistServer code path, but I have no live response.
8. **`docs/get-started/configuration.md`** 404'd at both `v0.57.0` and `main`; the `security.auth.selectedType` key is attested only via issues and the geminicli.com mirror, not first-party repo source at the pinned tag.

---

## RECOMMENDATION (for the taurhaus desktop coordinator)

**Multi-account: use `GEMINI_CLI_HOME` per spawned process.** Give each Google account its own root, e.g. `<TAURHAUS_DATA_DIR>/gemini-profiles/<label>/`, and spawn `gemini` with `GEMINI_CLI_HOME` set to that root. The CLI creates `<root>/.gemini/` and every credential, setting, and history file follows. This is the same pattern as `CODEX_HOME` / `CLAUDE_CONFIG_DIR`, so it fits the existing runtime-env plumbing. Do **not** set `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE` — it collapses all profiles onto a single keychain entry and breaks isolation.

**Account identity: read `<root>/.gemini/google_accounts.json` → `active`.** One tiny synchronous read, changes only on login/logout, no tokens involved. Pair it with `<root>/.gemini/settings.json` → `security.auth.selectedType` to decide whether a quota gauge is even meaningful for that profile. Show a "signed out" state when `active` is `null`.

**Quota: call `retrieveUserQuota` yourself, but only for `oauth-personal` profiles.**
`POST https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota`, `Authorization: Bearer <access_token>` from `<root>/.gemini/oauth_creds.json`, body `{"project": "<projectId>"}`. Cache the project ID per profile after one `loadCodeAssist` call. Render `remainingFraction` as the gauge and `resetTime` as the countdown — and treat `remainingAmount` as optional, because the API omits it at 100% (issue #27363); deriving the bar from `remainingAmount` alone reproduces that bug.

**Never refresh tokens from the coordinator.** Read `expiry_date`; if it is in the past, show the last known value with a staleness marker rather than minting a token. The CLI rewrites `oauth_creds.json` on its own refresh, and a competing refresh risks desyncing it. Poll on a slow cadence (60s+, and only while the Gemini panel is visible) — this is an undocumented internal API with 429 retry semantics, so be a good citizen.

**Gate the whole feature on auth mode.** `gemini-api-key` and `vertex-ai` have no Code Assist quota buckets at all; showing an empty or zeroed gauge there is worse than showing nothing. Render "usage not available for this auth mode" instead.

**Optional richer telemetry lane.** If you want burn-rate rather than remaining-quota, set `telemetry.enabled` + `telemetry.target: "local"` + `telemetry.outfile: <per-profile path>` in each profile's `settings.json`, and tail `gemini_cli.token.usage` records (attributes `model`, `type`, plus `session.id` / `user.email`). Note this writes multi-line pretty-printed JSON objects, so parse by object boundary, not by line. Treat this as a complement, not a replacement — it never yields remaining/limit/reset.

**Watch for drift:** `v1internal` is explicitly an internal API version and both the endpoint and version are env-overridable, which signals Google reserves the right to move them. Pin your expectations to `v0.57.0` shapes and fail soft (hide the gauge) on any unexpected response rather than surfacing an error to the user.
