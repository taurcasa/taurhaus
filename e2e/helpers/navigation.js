/**
 * Navigation helpers for E2E tests — tab switching, project selection, waits.
 *
 * PERF: All condition checks use browser.execute() to batch DOM queries into
 * a single WebDriver round-trip (~3ms) instead of multiple $() + isExisting()
 * calls (~100-500ms total). This is the single biggest speed optimization.
 */

import {
  POLL, POLL_FAST,
  TIMEOUT_MEDIUM,
  WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG,
} from './timing.js'

/**
 * Click an element by CSS selector using in-page JS (~3ms) instead of
 * WebDriver elementClick (~400ms). Use for any click on a known selector.
 * @param {string} selector - CSS selector
 */
export async function fastClick(selector) {
  await browser.execute((sel) => {
    document.querySelector(sel)?.click()
  }, selector)
}

/**
 * Click a data-testid element fast.
 * @param {string} testid - The data-testid value
 */
export async function clickTestId(testid) {
  await browser.execute((id) => {
    document.querySelector(`[data-testid="${id}"]`)?.click()
  }, testid)
}

/**
 * Switch to a tab by clicking its button and waiting for content.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 */
export async function switchToTab(tabName) {
  // Click in-page for speed (no WebDriver round-trip for the click itself)
  await browser.execute((name) => {
    document.querySelector(`[data-testid="tab-${name}"]`)?.click()
  }, tabName)
  await waitForTabContent(tabName)
}

/**
 * Wait for tab-specific content to appear after switching.
 * Single browser.execute() per poll — ~3ms instead of ~200-500ms.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 */
export async function waitForTabContent(tabName) {
  const selectorMap = {
    overview: ['[data-testid="quick-actions"]', '[data-testid="overview-readme"]'],
    files: ['[role="treeitem"]', '[data-testid="filetree-loading"]'],
    git: ['[data-testid="git-tab"]'],
    tasks: ['[data-testid="sub-tab-list"]', '[data-testid="tasks-loading"]'],
  }
  const selectors = selectorMap[tabName]
  if (!selectors) return

  await browser.waitUntil(
    async () => browser.execute(
      (sels) => sels.some(s => document.querySelector(s) !== null),
      selectors
    ),
    { ...WAIT_MEDIUM, timeoutMsg: `Tab content for "${tabName}" did not appear` }
  )
}

/**
 * Select a project by its name in the sidebar.
 * @param {string} name - Partial or full project name to match
 */
export async function selectProjectByName(name) {
  // Do the entire search + click in-page
  const clicked = await browser.execute((projectName) => {
    const items = document.querySelectorAll('[data-testid="project-item"]')
    for (const item of items) {
      if (item.textContent.includes(projectName)) {
        item.click()
        return true
      }
    }
    return false
  }, name)

  if (!clicked) return false

  // Wait for the h1 to update
  await browser.waitUntil(
    async () => browser.execute(
      (n) => document.querySelector('h1')?.textContent?.includes(n) ?? false,
      name
    ),
    { ...WAIT_MEDIUM, timeoutMsg: `Project "${name}" did not become active` }
  )
  return true
}

/**
 * Get the current project name from the Overview h1.
 * @returns {Promise<string>}
 */
export async function getCurrentProjectName() {
  return await browser.execute(() => document.querySelector('h1')?.textContent?.trim() ?? '')
}

/**
 * Wait for sidebar projects to load (skeleton disappears, projects appear).
 */
export async function waitForProjectsLoaded() {
  await browser.waitUntil(
    async () => browser.execute(() => {
      if (document.querySelector('[data-testid="sidebar-skeleton"]')) return false
      return document.querySelectorAll('[data-testid="project-item"]').length > 0
    }),
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
    async () => browser.execute(() =>
      document.querySelector('[data-testid="code-viewer"]') !== null ||
      document.querySelector('[data-testid="markdown-content"]') !== null
    ),
    { timeout, interval: POLL, timeoutMsg: msg }
  )
}

/**
 * Check whether a tab is currently active.
 * Single browser.execute() — ~3ms instead of 5 sequential WebDriver calls (~500ms).
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 * @returns {Promise<boolean>}
 */
export async function isTabActive(tabName) {
  return await browser.execute((name) => {
    const tab = document.querySelector(`[data-testid="tab-${name}"]`)
    if (!tab) return false
    if (tab.getAttribute('aria-selected') === 'true') return true
    if (tab.getAttribute('aria-current') === 'true' || tab.getAttribute('aria-current') === 'page') return true
    const cls = tab.className || ''
    return cls.includes('border-brand') || cls.includes('font-medium')
  }, tabName)
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
    async () => browser.execute(() =>
      document.querySelector('[data-testid="context-menu"]') !== null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Context menu did not appear after right-click' }
  )
}

/**
 * Dismiss the context menu if it's open.
 */
export async function dismissContextMenu() {
  await browser.execute(() => {
    if (document.querySelector('[data-testid="context-menu"]')) {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    }
  })
  // Brief wait for dismiss animation
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="context-menu"]') === null
    ),
    WAIT_SHORT
  )
}
