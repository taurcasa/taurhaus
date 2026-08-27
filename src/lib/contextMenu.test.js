import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte'
import { tick } from 'svelte'
import '@testing-library/jest-dom/vitest'

import ContextMenu from './ContextMenu.svelte'

function createItems(overrides = {}) {
  const copyAction = vi.fn()
  const renameAction = vi.fn()
  const removeAction = vi.fn()
  const pinAction = vi.fn()

  const items = [
    { label: 'Copy path', action: copyAction, icon: '<svg></svg>' },
    { label: 'Rename', action: renameAction, disabled: true },
    { separator: true },
    { label: 'Remove', action: removeAction, danger: true },
    { label: 'Pin', action: pinAction, keepOpen: true },
  ]

  return { items, copyAction, renameAction, removeAction, pinAction, ...overrides }
}

describe('ContextMenu', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders menu items, separator, and dark mode classes', () => {
    const { items } = createItems()

    render(ContextMenu, {
      props: {
        items,
        dark: true,
      },
    })

    const menu = screen.getByTestId('context-menu')
    expect(menu).toBeInTheDocument()
    expect(menu.className).toContain('bg-zinc-900')

    expect(screen.getByTestId('menu-item-copy-path')).toBeInTheDocument()
    expect(screen.getByTestId('menu-item-rename')).toBeDisabled()
    expect(screen.getByTestId('menu-item-remove').className).toContain('text-danger-500')
    expect(screen.getAllByRole('separator')).toHaveLength(1)
  })

  it('adjusts x/y position to remain inside viewport', async () => {
    const { items } = createItems()
    const prevWidth = window.innerWidth
    const prevHeight = window.innerHeight
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 800 })
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 600 })

    const { rerender } = render(ContextMenu, {
      props: {
        items,
        x: 790,
        y: 590,
      },
    })

    const menu = screen.getByTestId('context-menu')
    const rectSpy = vi
      .spyOn(menu, 'getBoundingClientRect')
      .mockReturnValue({
        left: 0,
        top: 0,
        right: 160,
        bottom: 120,
        width: 160,
        height: 120,
        x: 0,
        y: 0,
        toJSON() {},
      })

    await rerender({ items, x: 790, y: 590 })

    await waitFor(() => {
      expect(menu.style.left).toBe('632px')
      expect(menu.style.top).toBe('472px')
    })

    rectSpy.mockRestore()
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: prevWidth })
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: prevHeight })
  })

  it('clamps position to minimum 8px when negative coordinates are passed', async () => {
    const { items } = createItems()

    const { rerender } = render(ContextMenu, {
      props: {
        items,
        x: -50,
        y: -30,
      },
    })

    const menu = screen.getByTestId('context-menu')
    const rectSpy = vi
      .spyOn(menu, 'getBoundingClientRect')
      .mockReturnValue({
        left: 0,
        top: 0,
        right: 180,
        bottom: 140,
        width: 180,
        height: 140,
        x: 0,
        y: 0,
        toJSON() {},
      })

    await rerender({ items, x: -50, y: -30 })

    await waitFor(() => {
      expect(menu.style.left).toBe('8px')
      expect(menu.style.top).toBe('8px')
    })

    rectSpy.mockRestore()
  })

  it('invokes onClose on outside click but not for clicks on menu background', async () => {
    const { items } = createItems()
    const onClose = vi.fn()

    render(ContextMenu, {
      props: {
        items,
        onClose,
      },
    })

    await fireEvent.mouseDown(screen.getByTestId('context-menu'))
    expect(onClose).toHaveBeenCalledTimes(0)

    await fireEvent.mouseDown(document.body)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('handles disabled and enabled item clicks with keepOpen behavior', async () => {
    const { items, copyAction, renameAction, pinAction } = createItems()
    const onClose = vi.fn()

    render(ContextMenu, {
      props: {
        items,
        onClose,
      },
    })

    await fireEvent.mouseDown(screen.getByTestId('menu-item-rename'))
    expect(renameAction).not.toHaveBeenCalled()

    await fireEvent.mouseDown(screen.getByTestId('menu-item-copy-path'))
    expect(copyAction).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)

    await fireEvent.mouseDown(screen.getByTestId('menu-item-pin'))
    expect(pinAction).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('supports keyboard navigation, enter selection, and escape close', async () => {
    const { items, copyAction, removeAction, pinAction } = createItems()
    const onClose = vi.fn()

    render(ContextMenu, {
      props: {
        items,
        onClose,
      },
    })

    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'Enter' })

    expect(copyAction).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)

    await fireEvent.keyDown(window, { key: 'ArrowUp' })
    await fireEvent.keyDown(window, { key: ' ' })

    expect(pinAction).toHaveBeenCalledTimes(1)
    expect(removeAction).toHaveBeenCalledTimes(0)
    expect(onClose).toHaveBeenCalledTimes(1)

    await fireEvent.keyDown(window, { key: 'ArrowUp' })
    await fireEvent.keyDown(window, { key: ' ' })

    expect(removeAction).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(2)

    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(3)
  })

  it('skips disabled items while navigating with arrow keys', async () => {
    const onClose = vi.fn()
    const first = vi.fn()
    const third = vi.fn()

    render(ContextMenu, {
      props: {
        onClose,
        items: [
          { label: 'First', action: first },
          { label: 'Second', action: vi.fn(), disabled: true },
          { label: 'Third', action: third },
        ],
      },
    })

    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'Enter' })

    expect(third).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  // Regression: 74c7761 clamped the menu once, against the size it had when it
  // opened. The sidebar starts account detection when the menu opens and grows
  // the rows when the answer lands, so a menu opened near the bottom edge kept
  // its old top and pushed the new rows off the screen.
  it('re-clamps when rows arrive after the menu opened', async () => {
    const observers = []
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback) {
        this.callback = callback
        observers.push(this)
      }
      observe() {}
      disconnect() {}
    })
    const prevHeight = window.innerHeight
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 600 })

    render(ContextMenu, { props: { items: createItems().items, x: 100, y: 300 } })

    const menu = screen.getByTestId('context-menu')
    let height = 40
    vi.spyOn(menu, 'getBoundingClientRect').mockImplementation(() => ({
      left: 0, top: 0, right: 160, bottom: height, width: 160, height, x: 0, y: 0, toJSON() {},
    }))

    await waitFor(() => expect(menu.style.top).toBe('300px'))

    // The account rows landed: the same element, three times as tall.
    height = 320
    observers.at(-1).callback([])

    await waitFor(() => expect(menu.style.top).toBe('272px'))

    Object.defineProperty(window, 'innerHeight', { configurable: true, value: prevHeight })
    vi.unstubAllGlobals()
  })

  // Regression: 74c7761 repositioned only the flyout on a window resize, so the
  // root menu kept coordinates the viewport no longer had room for.
  it('re-clamps the root menu when the window resizes under it', async () => {
    const prevHeight = window.innerHeight
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 900 })

    render(ContextMenu, { props: { items: createItems().items, x: 100, y: 600 } })

    const menu = screen.getByTestId('context-menu')
    vi.spyOn(menu, 'getBoundingClientRect').mockReturnValue({
      left: 0, top: 0, right: 160, bottom: 200, width: 160, height: 200, x: 0, y: 0, toJSON() {},
    })
    await waitFor(() => expect(menu.style.top).toBe('600px'))

    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 500 })
    await fireEvent(window, new Event('resize'))

    await waitFor(() => expect(menu.style.top).toBe('292px'))

    Object.defineProperty(window, 'innerHeight', { configurable: true, value: prevHeight })
  })

  it('supports type-ahead navigation by item label', async () => {
    const onClose = vi.fn()
    const remove = vi.fn()

    render(ContextMenu, {
      props: {
        onClose,
        items: [
          { label: 'Copy path', action: vi.fn() },
          { label: 'Remove', action: remove },
          { label: 'Rename', action: vi.fn(), disabled: true },
        ],
      },
    })

    await fireEvent.keyDown(window, { key: 'r' })
    await fireEvent.keyDown(window, { key: 'Enter' })

    expect(remove).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})

describe('ContextMenu submenus', () => {
  const CHILDREN = [
    { label: 'Who', meta: '5h 3% · 7d 27%', check: true, action: vi.fn() },
    { label: 'Matthias', meta: '5h 61% · 7d 44%', action: vi.fn() },
    { label: 'Work', meta: 'not logged in', disabled: true, action: vi.fn() },
  ]

  function renderWithSubmenu(overrides = {}) {
    const onClose = vi.fn()
    const parentAction = vi.fn()
    const items = [
      { label: 'Copy path', action: vi.fn() },
      { label: 'New Claude Session', action: parentAction, children: CHILDREN },
      { label: 'Resume Claude', action: vi.fn(), children: CHILDREN },
      // A row that is nothing but its children: the pin submenu's shape.
      { label: 'Claude account', children: CHILDREN },
    ]
    render(ContextMenu, { props: { items, onClose, ...overrides } })
    return { onClose, parentAction }
  }

  /** Hover-open, which is how a parent that owns an action reveals its flyout. */
  async function hoverOpen(testid) {
    await fireEvent.mouseEnter(screen.getByTestId(testid))
    await vi.advanceTimersByTimeAsync(HOVER_INTENT_MS + 10)
  }

  const HOVER_INTENT_MS = 150

  function anchorRects(parent, flyout, { parentRect = {}, flyoutRect = {} } = {}) {
    vi.spyOn(parent, 'getBoundingClientRect').mockReturnValue({
      left: 100, top: 200, right: 260, bottom: 228, width: 160, height: 28, x: 100, y: 200,
      toJSON() {}, ...parentRect,
    })
    if (flyout) {
      vi.spyOn(flyout, 'getBoundingClientRect').mockReturnValue({
        left: 0, top: 0, right: 200, bottom: 120, width: 200, height: 120, x: 0, y: 0,
        toJSON() {}, ...flyoutRect,
      })
    }
  }

  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers({ shouldAdvanceTime: true })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('marks a parent row and opens a childrenonly row on click', async () => {
    renderWithSubmenu()

    const parent = screen.getByTestId('menu-item-new-claude-session')
    expect(parent).toHaveAttribute('aria-haspopup', 'menu')
    expect(parent).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByTestId('context-submenu')).not.toBeInTheDocument()

    const pinRow = screen.getByTestId('menu-item-claude-account')
    await fireEvent.mouseDown(pinRow)

    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()
    expect(pinRow).toHaveAttribute('aria-expanded', 'true')
  })

  // Regression: the account submenus must not turn a one-click launch into a
  // two-step. A parent that carries an action runs it on click; only its
  // hover flyout offers the accounts.
  it('runs the parent action on click instead of opening its flyout', async () => {
    const { onClose, parentAction } = renderWithSubmenu()

    await fireEvent.mouseDown(screen.getByTestId('menu-item-new-claude-session'))

    expect(parentAction).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(screen.queryByTestId('context-submenu')).not.toBeInTheDocument()
  })

  it('opens the flyout on hover only after the intent delay', async () => {
    renderWithSubmenu()

    await fireEvent.mouseEnter(screen.getByTestId('menu-item-new-claude-session'))
    expect(screen.queryByTestId('context-submenu')).not.toBeInTheDocument()

    await vi.advanceTimersByTimeAsync(160)

    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()
  })

  it('keeps the flyout open while the pointer crosses into it', async () => {
    renderWithSubmenu()

    const parent = screen.getByTestId('menu-item-new-claude-session')
    await hoverOpen('menu-item-new-claude-session')
    await fireEvent.mouseLeave(parent)

    // Mid-corridor: the pointer is between the row and the flyout.
    await vi.advanceTimersByTimeAsync(120)
    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()

    await fireEvent.mouseEnter(screen.getByTestId('context-submenu'))
    await vi.advanceTimersByTimeAsync(400)

    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()
  })

  it('closes the flyout when the pointer leaves it without coming back', async () => {
    renderWithSubmenu()

    const parent = screen.getByTestId('menu-item-new-claude-session')
    await hoverOpen('menu-item-new-claude-session')
    await fireEvent.mouseLeave(parent)
    await vi.advanceTimersByTimeAsync(260)

    expect(screen.queryByTestId('context-submenu')).not.toBeInTheDocument()
  })

  it('opens with ArrowRight and closes with ArrowLeft and Escape', async () => {
    const { onClose } = renderWithSubmenu()

    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowRight' })

    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()

    await fireEvent.keyDown(window, { key: 'ArrowLeft' })
    expect(screen.queryByTestId('context-submenu')).not.toBeInTheDocument()
    expect(onClose).not.toHaveBeenCalled()

    await fireEvent.keyDown(window, { key: 'ArrowRight' })
    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()

    // Escape closes the flyout only; the root menu is still the user's context.
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByTestId('context-submenu')).not.toBeInTheDocument()
    expect(onClose).not.toHaveBeenCalled()

    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('renders the check column and the meta text on every child', async () => {
    renderWithSubmenu()

    await hoverOpen('menu-item-new-claude-session')

    expect(screen.getByTestId('submenu-item-who')).toHaveTextContent('5h 3% · 7d 27%')
    expect(screen.getByTestId('submenu-check-who')).toHaveAttribute('data-checked', 'true')
    expect(screen.getByTestId('submenu-check-matthias')).toHaveAttribute('data-checked', 'false')
    expect(screen.getByTestId('submenu-item-work')).toBeDisabled()
    expect(screen.getByTestId('submenu-item-work')).toHaveTextContent('not logged in')
  })

  it('activates a child with the keyboard and closes the whole menu', async () => {
    const { onClose } = renderWithSubmenu()

    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowRight' })
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'Enter' })

    expect(CHILDREN[1].action).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('activates a child by click without running the parent action', async () => {
    const { onClose, parentAction } = renderWithSubmenu()

    await hoverOpen('menu-item-new-claude-session')
    await fireEvent.mouseDown(screen.getByTestId('submenu-item-matthias'))

    expect(CHILDREN[1].action).toHaveBeenCalledTimes(1)
    expect(parentAction).not.toHaveBeenCalled()
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('keeps one flyout open at a time', async () => {
    renderWithSubmenu()

    await hoverOpen('menu-item-new-claude-session')
    await fireEvent.mouseLeave(screen.getByTestId('menu-item-new-claude-session'))
    await hoverOpen('menu-item-resume-claude')

    expect(screen.getAllByTestId('context-submenu')).toHaveLength(1)
    expect(screen.getByTestId('menu-item-new-claude-session')).toHaveAttribute(
      'aria-expanded', 'false'
    )
    expect(screen.getByTestId('menu-item-resume-claude')).toHaveAttribute('aria-expanded', 'true')
  })

  it('places the flyout beside the parent row and flips it left when the viewport is narrow', async () => {
    const previousWidth = window.innerWidth
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1200 })

    renderWithSubmenu()
    const parent = screen.getByTestId('menu-item-new-claude-session')
    anchorRects(parent, null, {
      parentRect: { left: 400, right: 560, width: 160, x: 400 },
    })

    await hoverOpen('menu-item-new-claude-session')
    const flyout = screen.getByTestId('context-submenu')
    vi.spyOn(flyout, 'getBoundingClientRect').mockReturnValue({
      left: 0, top: 0, right: 200, bottom: 120, width: 200, height: 120, x: 0, y: 0,
      toJSON() {},
    })
    await fireEvent(window, new Event('resize'))

    // Wide window: the flyout sits against the row's right edge.
    expect(Number.parseInt(flyout.style.left, 10)).toBe(558)

    // Narrow window: no room on the right, so it flips to the row's left edge.
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 600 })
    await fireEvent(window, new Event('resize'))

    expect(Number.parseInt(flyout.style.left, 10)).toBe(202)

    Object.defineProperty(window, 'innerWidth', { configurable: true, value: previousWidth })
  })

  // Regression: 74c7761 keyed the flyout rows by `child.label`, which the
  // account rows derive from a display name two subscriptions can share. Svelte
  // threw `each_key_duplicate` and the whole submenu failed to render.
  it('renders children that share a label', async () => {
    const items = [{
      label: 'New Claude Session',
      action: vi.fn(),
      children: [
        { key: '0:account-1', label: 'Matthias', meta: '5h 3%', action: vi.fn() },
        { key: '1:account-2', label: 'Matthias', meta: '5h 61%', action: vi.fn() },
      ],
    }]
    render(ContextMenu, { props: { items, onClose: vi.fn() } })

    await hoverOpen('menu-item-new-claude-session')

    const rows = screen.getAllByRole('menuitemradio')
    expect(rows).toHaveLength(2)
    expect(rows[1]).toHaveTextContent('5h 61%')
  })

  // Regression: 6ec843e watched only the root menu for late growth. The rows
  // the sidebar waits for are the flyout's — it asks for the accounts when the
  // menu opens — so a flyout opened near the bottom edge grew past the edge and
  // stayed there, which is the cut-off popup the user reported.
  it('re-clamps the flyout when its rows arrive after it opened', async () => {
    const observers = []
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback) {
        this.callback = callback
        this.targets = []
        observers.push(this)
      }
      observe(target) { this.targets.push(target) }
      disconnect() {}
    })
    const previousHeight = window.innerHeight
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 600 })

    const parentItem = { label: 'New Claude Session', action: vi.fn(), children: [CHILDREN[0]] }
    const props = { items: [parentItem], onClose: vi.fn(), openChildOf: 'New Claude Session' }
    const { rerender } = render(ContextMenu, { props })

    const parent = screen.getByTestId('menu-item-new-claude-session')
    vi.spyOn(parent, 'getBoundingClientRect').mockReturnValue({
      left: 100, top: 400, right: 260, bottom: 428, width: 160, height: 28, x: 100, y: 400,
      toJSON() {},
    })
    const flyout = screen.getByTestId('context-submenu')
    let height = 60
    vi.spyOn(flyout, 'getBoundingClientRect').mockImplementation(() => ({
      left: 0, top: 0, right: 200, bottom: height, width: 200, height, x: 0, y: 0, toJSON() {},
    }))

    await fireEvent(window, new Event('resize'))
    expect(flyout.style.top).toBe('396px')

    // The accounts landed: three rows in the same flyout, five times as tall.
    // The root menu did not move, so nothing else asks for the clamp again.
    await rerender({ ...props, items: [{ ...parentItem, children: CHILDREN }] })
    height = 300
    const watcher = observers.find((observer) => observer.targets.includes(flyout))
    expect(watcher).toBeDefined()
    watcher.callback([])
    await tick()

    expect(flyout.style.top).toBe('292px')

    Object.defineProperty(window, 'innerHeight', { configurable: true, value: previousHeight })
    vi.unstubAllGlobals()
  })

  // Regression: 74c7761 clamped only the flyout's top edge. A flyout with more
  // rows than the viewport is tall — the accounts arriving late is exactly that
  // case — kept its full height, so its bottom rows sat off-screen with no way
  // to reach them.
  it('never grows past the viewport, and scrolls the focused row into view', async () => {
    const scrollIntoView = vi.fn()
    Element.prototype.scrollIntoView = scrollIntoView
    const previousHeight = window.innerHeight
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 300 })

    const children = Array.from({ length: 12 }, (_, index) => ({
      key: `${index}`,
      label: `Account ${index}`,
      meta: '5h 3%',
      action: vi.fn(),
    }))
    render(ContextMenu, {
      props: {
        items: [{ label: 'New Claude Session', action: vi.fn(), children }],
        onClose: vi.fn(),
        openChildOf: 'New Claude Session',
      },
    })

    const parent = screen.getByTestId('menu-item-new-claude-session')
    const flyout = screen.getByTestId('context-submenu')
    anchorRects(parent, flyout, {
      parentRect: { top: 40, bottom: 68, y: 40 },
      flyoutRect: { height: 800, bottom: 800 },
    })
    await fireEvent(window, new Event('resize'))

    expect(Number.parseFloat(flyout.style.maxHeight)).toBeLessThanOrEqual(window.innerHeight - 16)
    expect(flyout.style.overflowY).toBe('auto')

    // The last row is below the fold, so reaching it has to scroll the flyout.
    const lastRow = vi.spyOn(screen.getByTestId('submenu-item-account-11'), 'scrollIntoView')
    for (let i = 0; i < children.length; i++) {
      await fireEvent.keyDown(window, { key: 'ArrowDown' })
    }
    await tick()

    expect(lastRow).toHaveBeenCalledWith({ block: 'nearest' })

    Object.defineProperty(window, 'innerHeight', { configurable: true, value: previousHeight })
    delete Element.prototype.scrollIntoView
  })

  // Regression: 6ec843e treated ArrowRight inside an open flyout like Enter. The
  // first ArrowRight opens the accounts and the key repeat of a held press then
  // picked the first one — on a restart row that stops a live session.
  it('does not activate a child on a repeated ArrowRight', async () => {
    const { onClose } = renderWithSubmenu()

    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowRight' })
    await fireEvent.keyDown(window, { key: 'ArrowRight' })
    await fireEvent.keyDown(window, { key: 'ArrowRight' })

    expect(CHILDREN[0].action).not.toHaveBeenCalled()
    expect(CHILDREN[1].action).not.toHaveBeenCalled()
    expect(onClose).not.toHaveBeenCalled()
    // The flyout is still the level the user is on.
    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()
  })

  it('searches the open level with typeahead', async () => {
    renderWithSubmenu()

    await hoverOpen('menu-item-new-claude-session')
    await fireEvent.keyDown(window, { key: 'm' })
    await fireEvent.keyDown(window, { key: 'Enter' })

    expect(CHILDREN[1].action).toHaveBeenCalledTimes(1)
  })
})
