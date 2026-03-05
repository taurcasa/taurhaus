import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte'
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
})
