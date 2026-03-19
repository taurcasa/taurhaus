const FOCUSABLE_SELECTOR = [
  'a[href]',
  'area[href]',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'button:not([disabled])',
  'iframe',
  'object',
  'embed',
  '[contenteditable="true"]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

const activeModalStack = []
const managedInertNodes = new Set()

function cleanupDetachedModals() {
  for (let index = activeModalStack.length - 1; index >= 0; index -= 1) {
    const element = activeModalStack[index]
    if (!(element instanceof HTMLElement) || !element.isConnected) {
      activeModalStack.splice(index, 1)
    }
  }
}

function clearManagedInertNodes() {
  for (const node of managedInertNodes) {
    if (!node.isConnected) continue
    if (node.getAttribute('data-managed-inert') !== 'true') continue
    node.removeAttribute('inert')
    node.removeAttribute('aria-hidden')
    node.removeAttribute('data-managed-inert')
  }
  managedInertNodes.clear()
}

function markManagedInert(node) {
  if (!(node instanceof HTMLElement)) return
  if (node.getAttribute('data-managed-inert') === 'true') return
  node.setAttribute('inert', '')
  node.setAttribute('aria-hidden', 'true')
  node.setAttribute('data-managed-inert', 'true')
  managedInertNodes.add(node)
}

function applyModalIsolation() {
  if (typeof document === 'undefined') return

  cleanupDetachedModals()
  clearManagedInertNodes()

  const activeModal = activeModalStack.at(-1)
  if (!(activeModal instanceof HTMLElement) || !activeModal.isConnected) return

  let current = activeModal
  while (current && current !== document.body) {
    const parent = current.parentElement
    if (!parent) break

    for (const sibling of parent.children) {
      if (sibling !== current) {
        markManagedInert(sibling)
      }
    }

    current = parent
  }
}

export function getFocusableElements(container) {
  if (!(container instanceof HTMLElement)) return []
  return Array.from(container.querySelectorAll(FOCUSABLE_SELECTOR))
    .filter((element) => (
      element instanceof HTMLElement
      && !element.hasAttribute('disabled')
      && element.tabIndex >= 0
      && element.getAttribute('aria-hidden') !== 'true'
    ))
}

export function focusFirstInteractiveElement(container, preferredTarget = null) {
  const preferred = typeof preferredTarget === 'function'
    ? preferredTarget()
    : preferredTarget

  if (preferred instanceof HTMLElement && preferred.isConnected && !preferred.hasAttribute('disabled')) {
    preferred.focus()
    return
  }

  const [firstFocusable] = getFocusableElements(container)
  if (firstFocusable) {
    firstFocusable.focus()
    return
  }

  if (container instanceof HTMLElement) {
    container.focus()
  }
}

export function handleModalKeydown(event, container, onClose = null) {
  if (!(container instanceof HTMLElement)) return

  if (event.key === 'Escape') {
    event.preventDefault()
    onClose?.()
    return
  }

  if (event.key !== 'Tab') return

  const focusable = getFocusableElements(container)
  if (focusable.length === 0) {
    event.preventDefault()
    container.focus()
    return
  }

  const first = focusable[0]
  const last = focusable[focusable.length - 1]
  const active = document.activeElement
  const activeInside = active instanceof HTMLElement && container.contains(active)

  if (!activeInside) {
    event.preventDefault()
    ;(event.shiftKey ? last : first).focus()
    return
  }

  if (!event.shiftKey && active === last) {
    event.preventDefault()
    first.focus()
    return
  }

  if (event.shiftKey && active === first) {
    event.preventDefault()
    last.focus()
  }
}

export function registerModalLayer(rootElement) {
  if (!(rootElement instanceof HTMLElement) || typeof document === 'undefined') {
    return () => {}
  }

  activeModalStack.push(rootElement)
  applyModalIsolation()

  return () => {
    const index = activeModalStack.lastIndexOf(rootElement)
    if (index !== -1) {
      activeModalStack.splice(index, 1)
    }
    applyModalIsolation()
  }
}

export function isContextMenuKey(event) {
  return event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')
}

export function getContextMenuPoint(element) {
  if (!(element instanceof HTMLElement)) {
    return { x: 8, y: 8 }
  }

  const rect = element.getBoundingClientRect()
  return {
    x: Math.max(8, rect.left + Math.min(24, rect.width / 2)),
    y: Math.max(8, rect.top + Math.min(24, rect.height / 2)),
  }
}
