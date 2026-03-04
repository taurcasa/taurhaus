export async function isSlideOverOpen() {
  return await browser.execute(() => {
    const roots = Array.from(document.querySelectorAll('[data-testid="slideover-root"]'))
    const visibleRoots = roots.filter((root) => {
      if (!(root instanceof HTMLElement)) return false
      const style = window.getComputedStyle(root)
      if (style.display === 'none' || style.visibility === 'hidden') return false
      return root.getClientRects().length > 0
    })
    return (visibleRoots.at(-1) ?? roots.at(-1) ?? null) !== null
  })
}

export async function hasActiveSlideOverTestId(testId) {
  return await browser.execute((id) => {
    const roots = Array.from(document.querySelectorAll('[data-testid="slideover-root"]'))
    const visibleRoots = roots.filter((root) => {
      if (!(root instanceof HTMLElement)) return false
      const style = window.getComputedStyle(root)
      if (style.display === 'none' || style.visibility === 'hidden') return false
      return root.getClientRects().length > 0
    })
    const root = visibleRoots.at(-1) ?? roots.at(-1) ?? null
    if (!root) return false
    return root.querySelector(`[data-testid="${id}"]`) !== null
  }, testId)
}

export async function clickActiveSlideOverTestId(testId) {
  return await browser.execute((id) => {
    const roots = Array.from(document.querySelectorAll('[data-testid="slideover-root"]'))
    const visibleRoots = roots.filter((root) => {
      if (!(root instanceof HTMLElement)) return false
      const style = window.getComputedStyle(root)
      if (style.display === 'none' || style.visibility === 'hidden') return false
      return root.getClientRects().length > 0
    })
    const root = visibleRoots.at(-1) ?? roots.at(-1) ?? null
    if (!root) return false
    const target = root.querySelector(`[data-testid="${id}"]`)
    if (!(target instanceof HTMLElement)) return false
    target.click()
    return true
  }, testId)
}

export async function setActiveSlideOverInputValue(testId, value) {
  return await browser.execute((id, nextValue) => {
    const roots = Array.from(document.querySelectorAll('[data-testid="slideover-root"]'))
    const visibleRoots = roots.filter((root) => {
      if (!(root instanceof HTMLElement)) return false
      const style = window.getComputedStyle(root)
      if (style.display === 'none' || style.visibility === 'hidden') return false
      return root.getClientRects().length > 0
    })
    const root = visibleRoots.at(-1) ?? roots.at(-1) ?? null
    if (!root) return false
    const target = root.querySelector(`[data-testid="${id}"]`)
    if (!(target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement)) return false

    target.focus()
    target.value = ''
    target.dispatchEvent(new Event('input', { bubbles: true }))
    target.value = String(nextValue ?? '')
    target.dispatchEvent(new Event('input', { bubbles: true }))
    target.dispatchEvent(new Event('change', { bubbles: true }))
    return true
  }, testId, value)
}

export async function readActiveSlideOverInputValue(testId) {
  return await browser.execute((id) => {
    const roots = Array.from(document.querySelectorAll('[data-testid="slideover-root"]'))
    const visibleRoots = roots.filter((root) => {
      if (!(root instanceof HTMLElement)) return false
      const style = window.getComputedStyle(root)
      if (style.display === 'none' || style.visibility === 'hidden') return false
      return root.getClientRects().length > 0
    })
    const root = visibleRoots.at(-1) ?? roots.at(-1) ?? null
    if (!root) return null
    const target = root.querySelector(`[data-testid="${id}"]`)
    if (!(target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement)) {
      return null
    }
    return String(target.value ?? '')
  }, testId)
}
