import { existsSync } from 'node:fs'
import { join } from 'node:path'

/** Inspect the current Codex state database without creating or opening it. */
export function codexStateDatabaseDiagnostic(codexHome) {
  const path = join(codexHome, 'state_5.sqlite')
  return { path, exists: existsSync(path) }
}

/** Launch the first managed member with its team, then hot-add the rest. */
export async function launchManagedMembersSerially({ members, initialize, add, waitForBinding }) {
  const [first, ...remaining] = members
  if (!first) return

  await initialize(first)
  await waitForBinding(first)

  for (const member of remaining) {
    await add(member)
    await waitForBinding(member)
  }
}
