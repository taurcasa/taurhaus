import { clickTestId } from './navigation.js'

/**
 * Retry a lost runtime click until its target state holds. Strings are test IDs;
 * callbacks allow named nodes / open-dialog predicates and must re-query DOM.
 * Only for repeatable UI controls; never pass a launch, save or confirmation.
 * Audit and cadence evidence: docs/operations/mesh-flake-audit.md.
 */
export async function clickUntil(click, target, wait) {
  const isOpen = typeof target === 'function'
    ? target
    : async () => await (await $(`[data-testid="${target}"]`)).isExisting()
  const attempt = typeof click === 'function' ? click : () => clickTestId(click)
  await browser.waitUntil(async () => {
    // Regression: 430e09ee used single runtime clicks that the live-status
    // poll could lose. Check first so a successful menu toggle is not undone.
    if (await isOpen()) return true
    await attempt()
    return await isOpen()
  }, wait)
}
