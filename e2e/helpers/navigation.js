/**
 * Navigation helpers for E2E tests — tab switching, project selection, waits.
 */

import {
  POLL, POLL_FAST,
  TIMEOUT_MEDIUM,
  WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG,
} from './timing.js'

/**
 * Click an element by CSS selector.
 * @param {string} selector - CSS selector
 */
export async function fastClick(selector) {
  const el = await $(selector)
  if (!(await el.isExisting())) return false

  await el.scrollIntoView().catch(() => {})
  const clicked = await el.click()
    .then(() => true)
    .catch(async (error) => {
      const message = String(error?.message ?? '').toLowerCase()
      const recoverable =
        message.includes('intercepted') ||
        message.includes('not interactable') ||
        message.includes('stale element')
      if (!recoverable) throw error

      return await browser.execute((sel) => {
        const target = document.querySelector(sel)
        if (!target) return false
        target.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
        target.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }))
        target.dispatchEvent(new MouseEvent('click', { bubbles: true }))
        return true
      }, selector).catch(() => false)
    })

  return clicked
}

/**
 * Click a data-testid element fast.
 * @param {string} testid - The data-testid value
 */
export async function clickTestId(testid) {
  await fastClick(`[data-testid="${testid}"]`)
}

/**
 * Switch to a tab by clicking its button and waiting for content.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 */
export async function switchToTab(tabName) {
  await clickTestId(`tab-${tabName}`)
  await waitForTabContent(tabName)
}

/**
 * Wait for tab-specific content to appear after switching.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 */
async function waitForTabContent(tabName) {
  const selectorMap = {
    overview: ['[data-testid="quick-actions"]', '[data-testid="overview-readme"]'],
    files: [
      '[data-testid="file-tree-node"]',
      '[data-testid="filetree-loading"]',
      '[data-testid="file-tree-scroll"]',
    ],
    git: ['[data-testid="git-tab"]'],
    tasks: ['[data-testid="sub-tab-list"]', '[data-testid="tasks-loading"]'],
  }
  const selectors = selectorMap[tabName]
  if (!selectors) return

  await browser.waitUntil(
    async () => {
      for (const selector of selectors) {
        const el = await $(selector)
        if (await el.isExisting()) return true
      }
      return false
    },
    { ...WAIT_MEDIUM, timeoutMsg: `Tab content for "${tabName}" did not appear` }
  )
}

/**
 * Select a project by its name in the sidebar.
 * @param {string} name - Partial or full project name to match
 */
export async function selectProjectByName(name) {
  const items = await $$('[data-testid="project-item"]')
  let clicked = false
  for (const item of items) {
    const text = await item.getText()
    if (text.includes(name)) {
      await item.click()
      clicked = true
      break
    }
  }

  if (!clicked) return false

  // Wait for the h1 to update
  await browser.waitUntil(
    async () => {
      const h1 = await $('h1')
      if (!(await h1.isExisting())) return false
      return (await h1.getText()).includes(name)
    },
    { ...WAIT_MEDIUM, timeoutMsg: `Project "${name}" did not become active` }
  )
  return true
}

/**
 * Get the current project name from the Overview h1.
 * @returns {Promise<string>}
 */
export async function getCurrentProjectName() {
  const h1 = await $('h1')
  if (!(await h1.isExisting())) return ''
  return (await h1.getText()).trim()
}

/**
 * Wait for sidebar projects to load (skeleton disappears, projects appear).
 */
export async function waitForProjectsLoaded() {
  const skeleton = await $('[data-testid="sidebar-skeleton"]')
  if (await skeleton.isExisting()) {
    await skeleton.waitForExist({ ...WAIT_LONG, reverse: true })
  }

  await browser.waitUntil(
    async () => (await $$('[data-testid="project-item"]')).length > 0,
    { ...WAIT_LONG, timeoutMsg: 'Projects did not load in sidebar' }
  )
}

/**
 * Wait for file content to appear (code-viewer or markdown-content).
 * @param {number} [timeout=TIMEOUT_MEDIUM]
 * @param {string} [msg='File content did not appear']
 */
export async function waitForFileContent(timeout = TIMEOUT_MEDIUM, msg = 'File content did not appear') {
  await browser.waitUntil(
    async () => {
      const code = await $('[data-testid="code-viewer"]')
      if (await code.isExisting()) return true
      const markdown = await $('[data-testid="markdown-content"]')
      return await markdown.isExisting()
    },
    { timeout, interval: POLL, timeoutMsg: msg }
  )
}

/**
 * Check whether a tab is currently active.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 * @returns {Promise<boolean>}
 */
async function isTabActive(tabName) {
  const tab = await $(`[data-testid="tab-${tabName}"]`)
  if (!(await tab.isExisting())) return false
  const ariaSelected = await tab.getAttribute('aria-selected')
  if (ariaSelected === 'true') return true
  const ariaCurrent = await tab.getAttribute('aria-current')
  if (ariaCurrent === 'true' || ariaCurrent === 'page') return true
  const cls = await tab.getAttribute('class')
  return (cls || '').includes('border-brand') || (cls || '').includes('font-medium')
}

/**
 * Wait for a tab to become active.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 * @param {number} [timeout=TIMEOUT_MEDIUM]
 */
export async function waitForTabActive(tabName, timeout = TIMEOUT_MEDIUM) {
  await browser.waitUntil(
    async () => await isTabActive(tabName),
    { timeout, interval: POLL, timeoutMsg: `Tab "${tabName}" did not become active` }
  )
}

/**
 * Right-click an element and wait for the context menu to appear.
 * @param {WebdriverIO.Element} element
 */
export async function openContextMenu(element) {
  await element.click({ button: 'right' })
  await browser.waitUntil(
    async () => (await $('[data-testid="context-menu"]')).isExisting(),
    { ...WAIT_MEDIUM, timeoutMsg: 'Context menu did not appear after right-click' }
  )
}

/**
 * Dismiss the context menu if it's open.
 */
export async function dismissContextMenu() {
  const menu = await $('[data-testid="context-menu"]')
  if (!(await menu.isExisting())) return

  await browser.keys('Escape')

  // Best-effort close: if Escape is ignored, click outside and continue.
  const closedAfterEscape = await browser.waitUntil(
    async () => !(await $('[data-testid="context-menu"]')).isExisting(),
    WAIT_SHORT
  ).catch(() => false)

  if (closedAfterEscape) return

  await browser.execute(() => {
    const root = document.querySelector('main') || document.body
    root?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
    root?.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }))
    root?.dispatchEvent(new MouseEvent('click', { bubbles: true }))
  }).catch(() => {})

  await browser.waitUntil(
    async () => !(await $('[data-testid="context-menu"]')).isExisting(),
    WAIT_SHORT
  ).catch(() => {})
}
