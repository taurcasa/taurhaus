// Static check for the versioned Workflow scripts in .claude/workflows.
// Parses each script (no execution) and enforces the constraints the Workflow
// runtime puts on them: `export const meta` first, no imports, and none of the
// globals that throw inside a workflow script.
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { fileURLToPath } from 'node:url'

export const DEFAULT_WORKFLOWS_DIR = '.claude/workflows'

// Workflow scripts run as one inline script with the API injected as globals.
const BANNED = [
  [/^[ \t]*import\s+[^(]/m, 'uses `import` — workflow scripts cannot import; copy the shared lib section instead'],
  [/\brequire\s*\(/, 'uses `require(` — workflow scripts cannot import; copy the shared lib section instead'],
  [/\bDate\.now\s*\(/, 'uses `Date.now()` — it throws in a workflow script (it would break resume); pass a timestamp via args'],
  [/\bnew\s+Date\s*\(\s*\)/, 'uses argless `new Date()` — it throws in a workflow script; pass a timestamp via args'],
  [/\bMath\.random\s*\(/, 'uses `Math.random()` — it throws in a workflow script; vary the prompt or label by index instead'],
]

// Strips whitespace and leading comments so `export const meta` can be checked
// as the first statement without pulling in a parser.
function withoutLeadingTrivia(source) {
  let rest = source
  for (;;) {
    const trimmed = rest.replace(/^\s+/, '')
    if (trimmed.startsWith('//')) {
      rest = trimmed.slice(trimmed.indexOf('\n') + 1)
      continue
    }
    if (trimmed.startsWith('/*')) {
      const end = trimmed.indexOf('*/')
      if (end === -1) return trimmed
      rest = trimmed.slice(end + 2)
      continue
    }
    return trimmed
  }
}

// Wraps the script so a parser accepts what the Workflow runtime accepts:
// top-level `await`, top-level `return`, and the single `export const meta`.
export function wrapForParse(source) {
  const head = withoutLeadingTrivia(source)
  const trivia = source.slice(0, source.length - head.length)
  const body = /^export\s+const\s+meta\b/.test(head) ? trivia + head.replace(/^export\s+/, '') : source
  return `(async function workflowScript(agent, parallel, pipeline, phase, log, workflow, args, budget) {\n${body}\n})`
}

function syntaxProblem(name, source) {
  try {
    // Compiles only — vm.Script never runs the script.
    new vm.Script(wrapForParse(source), { filename: name, lineOffset: -1 })
    return null
  } catch (error) {
    const location = String(error.stack || '').split('\n')[0]
    const line = /:(\d+)$/.exec(location)
    return `${name}${line ? `:${line[1]}` : ''} syntax error: ${error.message}`
  }
}

export function checkWorkflowSource(fileName, source) {
  const problems = []
  const syntax = syntaxProblem(fileName, source)
  if (syntax) problems.push(syntax)

  const head = withoutLeadingTrivia(source)
  if (!/^export\s+const\s+meta\s*=\s*\{/.test(head)) {
    problems.push(`${fileName}: must start with \`export const meta = {\` (the runtime reads meta before running the script)`)
    return problems
  }

  const expected = path.basename(fileName, '.js')
  const declaredName = /\bname:\s*(['"])([^'"]+)\1/.exec(head)
  if (!declaredName) {
    problems.push(`${fileName}: meta.name is missing`)
  } else if (declaredName[2] !== expected) {
    problems.push(`${fileName}: meta.name '${declaredName[2]}' does not match the file name '${expected}'`)
  }
  if (!/\bdescription:\s*['"]/.test(head)) {
    problems.push(`${fileName}: meta.description is missing (it is shown in the permission dialog)`)
  }

  for (const [pattern, message] of BANNED) {
    if (pattern.test(source)) problems.push(`${fileName}: ${message}`)
  }
  return problems
}

export function checkWorkflowDir(dir) {
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    return { checked: 0, problems: [`${dir}: workflow directory not found`] }
  }
  const files = fs
    .readdirSync(dir)
    .filter((file) => file.endsWith('.js'))
    .sort()
  if (files.length === 0) {
    return { checked: 0, problems: [`${dir}: no workflow scripts found`] }
  }
  const problems = files.flatMap((file) => checkWorkflowSource(file, fs.readFileSync(path.join(dir, file), 'utf8')))
  return { checked: files.length, problems }
}

function main() {
  const dir = process.argv[2] || DEFAULT_WORKFLOWS_DIR
  const { checked, problems } = checkWorkflowDir(dir)
  if (problems.length > 0) {
    for (const problem of problems) console.error(problem)
    console.error(`workflow script check failed: ${problems.length} problem(s) in ${dir}`)
    process.exit(1)
  }
  console.log(`workflow script check passed: ${checked} script(s) in ${dir}`)
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (import.meta.main === true || invokedPath === fileURLToPath(import.meta.url)) {
  main()
}
