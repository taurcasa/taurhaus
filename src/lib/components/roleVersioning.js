export const ROLE_VERSION_VISIBILITY_STORAGE_KEY =
  'taurhaus.mesh.builder.show-all-role-versions'

const VERSION_PREFIX_RE = /^v(\d+)[-_ ]+/i

function slugify(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
}

export function stripRoleVersionPrefix(value) {
  return String(value ?? '').trim().replace(VERSION_PREFIX_RE, '')
}

export function parseRoleVersionNumber(role) {
  const candidates = [role?.roleId, role?.name]
  for (const value of candidates) {
    const match = String(value ?? '').trim().match(VERSION_PREFIX_RE)
    if (match) {
      return Number(match[1]) || 0
    }
  }
  return 0
}

function stripToolPrefix(value, cliTool) {
  const normalizedValue = slugify(value)
  const tool = slugify(cliTool)
  if (!normalizedValue || !tool) return normalizedValue
  if (normalizedValue === tool) return normalizedValue
  const prefix = `${tool}-`
  return normalizedValue.startsWith(prefix)
    ? normalizedValue.slice(prefix.length)
    : normalizedValue
}

function roleFunctionIdentity(role) {
  const tool = role?.cliTool ?? ''
  const candidates = [role?.name, role?.roleId]
  for (const candidate of candidates) {
    const normalized = stripToolPrefix(stripRoleVersionPrefix(candidate), tool)
    if (normalized) return normalized
  }
  return 'role'
}

export function roleVersionGroupKey(role) {
  const tool = slugify(role?.cliTool) || 'unknown'
  const kind = String(role?.kind ?? 'agent').trim().toLowerCase() === 'lead' ? 'lead' : 'agent'
  return `${tool}:${kind}:${roleFunctionIdentity(role)}`
}

function parseRoleTimestamp(role) {
  const candidates = [
    role?.provenance?.importedAt,
    role?.provenance?.imported_at,
    role?.importedAt,
    role?.imported_at,
    role?.createdAt,
    role?.created_at,
    role?.updatedAt,
    role?.updated_at,
  ]

  for (const candidate of candidates) {
    const timestamp = Date.parse(String(candidate ?? ''))
    if (Number.isFinite(timestamp)) return timestamp
  }

  return 0
}

function compareRolePrecedence(candidate, current) {
  const versionDelta = parseRoleVersionNumber(candidate) - parseRoleVersionNumber(current)
  if (versionDelta !== 0) return versionDelta

  const timestampDelta = parseRoleTimestamp(candidate) - parseRoleTimestamp(current)
  if (timestampDelta !== 0) return timestampDelta

  return String(candidate?.roleId ?? '').localeCompare(String(current?.roleId ?? ''))
}

export function latestRoleVersions(roles = []) {
  const latestByGroup = new Map()

  for (const role of roles) {
    const existing = latestByGroup.get(roleVersionGroupKey(role))
    if (!existing || compareRolePrecedence(role, existing) > 0) {
      latestByGroup.set(roleVersionGroupKey(role), role)
    }
  }

  const visibleRoles = new Set(latestByGroup.values())
  return roles.filter((role) => visibleRoles.has(role))
}
