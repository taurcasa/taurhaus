// @vitest-environment node
import { execFileSync, spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const justfile = readFileSync('justfile', 'utf8')

// Regression: a42d8aa7ef preserved TAURHAUS_*/RUST_LOG but dropped PATH,
// undoing the interactive-shell resolution needed by dogfood finding 7.
describe('daemon install PATH preservation', () => {
  it('captures PATH verbatim from a synthetic NUL-delimited environment', () => {
    const loop = justfile.match(/while IFS= read -r -d '' kv; do[\s\S]*?done < "\/proc\/\$OLD_PID\/environ"/)[0]
      .replace(' < "/proc/$OLD_PID/environ"', '')
    const path = "/fixture/node bin:/fixture/quote'/$literal:\n/fixture/newline:"
    const output = execFileSync('bash', ['-c', `PRESERVED_ENV=(); ${loop}; printf '%s\\0' "\${PRESERVED_ENV[@]}"`], {
      input: `PATH=${path}\0RUST_LOG=info\0UNRELATED=ignored\0`,
      encoding: 'utf8',
    })
    expect(output.split('\0').filter(Boolean)).toEqual([`PATH=${path}`, 'RUST_LOG=info'])
  })

  it('exposes the shared PATH capture in install-daemon dry-run without printing environment values', () => {
    const dryRun = spawnSync('just', ['-n', 'install-daemon'], { encoding: 'utf8' })
    expect(dryRun.status).toBe(0)
    const result = dryRun.stdout + dryRun.stderr
    expect(result).toContain('|PATH=*) PRESERVED_ENV+=("$kv")')
    expect(result).toContain('done < "/proc/$OLD_PID/environ"')
    expect(result).not.toContain('${PRESERVED_ENV[*]')
  })
})
