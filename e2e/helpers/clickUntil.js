import { clickTestId } from './navigation.js'

/**
 * Retry a lost runtime click until its target opens. Strings are test IDs;
 * callbacks allow named nodes / open-dialog predicates and must re-query DOM.
 * Only for repeatable UI openers; never pass a launch, save or confirmation.
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
