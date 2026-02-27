import { describe, it, expect, vi } from 'vitest'

/**
 * ContextMenu component logic tests.
 *
 * Since Svelte 5 component rendering in vitest/jsdom has SSR issues,
 * we test the behavioral logic (item filtering, keyboard nav, positioning)
 * as pure functions, matching the project's existing test pattern.
 */

describe('ContextMenu logic', () => {
  // Helper: simulate the actionable items filter (mirrors component logic)
  function getActionableItems(items) {
    return items.filter(i => !i.separator)
  }

  describe('item filtering', () => {
    it('filters out separator items for keyboard nav', () => {
      const items = [
        { label: 'Copy path', action: vi.fn() },
        { separator: true },
        { label: 'Remove', action: vi.fn(), danger: true },
      ]
      const actionable = getActionableItems(items)
      expect(actionable).toHaveLength(2)
      expect(actionable[0].label).toBe('Copy path')
      expect(actionable[1].label).toBe('Remove')
    })

    it('handles empty items list', () => {
      const actionable = getActionableItems([])
      expect(actionable).toHaveLength(0)
    })

    it('handles all separators', () => {
      const items = [{ separator: true }, { separator: true }]
      const actionable = getActionableItems(items)
      expect(actionable).toHaveLength(0)
    })
  })

  describe('keyboard navigation logic', () => {
    // Simulate the ArrowDown logic from the component
    function navigateDown(focusIndex, actionableItems) {
      let next = focusIndex
      for (let i = 0; i < actionableItems.length; i++) {
        next = (next + 1) % actionableItems.length
        if (!actionableItems[next].disabled) break
      }
      return next
    }

    function navigateUp(focusIndex, actionableItems) {
      let prev = focusIndex
      for (let i = 0; i < actionableItems.length; i++) {
        prev = prev <= 0 ? actionableItems.length - 1 : prev - 1
        if (!actionableItems[prev].disabled) break
      }
      return prev
    }

    const items = [
      { label: 'A', action: vi.fn() },
      { label: 'B', action: vi.fn() },
      { label: 'C', action: vi.fn() },
    ]

    it('ArrowDown from -1 moves to first item', () => {
      expect(navigateDown(-1, items)).toBe(0)
    })

    it('ArrowDown from first moves to second', () => {
      expect(navigateDown(0, items)).toBe(1)
    })

    it('ArrowDown wraps from last to first', () => {
      expect(navigateDown(2, items)).toBe(0)
    })

    it('ArrowUp from first wraps to last', () => {
      expect(navigateUp(0, items)).toBe(2)
    })

    it('ArrowUp from second moves to first', () => {
      expect(navigateUp(1, items)).toBe(0)
    })

    it('skips disabled items going down', () => {
      const withDisabled = [
        { label: 'A', action: vi.fn() },
        { label: 'B', action: vi.fn(), disabled: true },
        { label: 'C', action: vi.fn() },
      ]
      expect(navigateDown(0, withDisabled)).toBe(2)
    })

    it('skips disabled items going up', () => {
      const withDisabled = [
        { label: 'A', action: vi.fn() },
        { label: 'B', action: vi.fn(), disabled: true },
        { label: 'C', action: vi.fn() },
      ]
      expect(navigateUp(2, withDisabled)).toBe(0)
    })
  })

  describe('viewport positioning logic', () => {
    // Simulate the positioning logic from the component
    function adjustPosition(x, y, menuWidth, menuHeight, vw, vh) {
      let nx = x
      let ny = y

      if (x + menuWidth > vw - 8) {
        nx = vw - menuWidth - 8
      }
      if (y + menuHeight > vh - 8) {
        ny = vh - menuHeight - 8
      }

      return {
        x: Math.max(8, nx),
        y: Math.max(8, ny),
      }
    }

    it('keeps position when menu fits', () => {
      const result = adjustPosition(100, 200, 160, 120, 1920, 1080)
      expect(result.x).toBe(100)
      expect(result.y).toBe(200)
    })

    it('shifts left when overflowing right edge', () => {
      const result = adjustPosition(1800, 100, 160, 120, 1920, 1080)
      expect(result.x).toBe(1920 - 160 - 8)
    })

    it('shifts up when overflowing bottom edge', () => {
      const result = adjustPosition(100, 980, 160, 120, 1920, 1080)
      expect(result.y).toBe(1080 - 120 - 8)
    })

    it('clamps to minimum of 8px from edge', () => {
      const result = adjustPosition(-100, -50, 160, 120, 1920, 1080)
      expect(result.x).toBe(8)
      expect(result.y).toBe(8)
    })
  })

  describe('item action handling', () => {
    it('disabled items should not call action', () => {
      const action = vi.fn()
      const item = { label: 'Test', action, disabled: true }

      // Simulate handleItemClick logic
      if (!item.disabled && item.action) {
        item.action()
      }

      expect(action).not.toHaveBeenCalled()
    })

    it('enabled items should call action', () => {
      const action = vi.fn()
      const item = { label: 'Test', action, disabled: false }

      if (!item.disabled && item.action) {
        item.action()
      }

      expect(action).toHaveBeenCalled()
    })

    it('danger flag is just metadata, action still fires', () => {
      const action = vi.fn()
      const item = { label: 'Remove', action, danger: true }

      if (!item.disabled && item.action) {
        item.action()
      }

      expect(action).toHaveBeenCalled()
    })

    it('keepOpen items call action but suppress close', () => {
      const action = vi.fn()
      const onClose = vi.fn()
      const item = { label: 'Remove', action, keepOpen: true }

      // Simulate handleItemClick logic (mirrors component)
      if (!item.disabled && item.action) {
        item.action()
      }
      if (!item.keepOpen) {
        onClose()
      }

      expect(action).toHaveBeenCalled()
      expect(onClose).not.toHaveBeenCalled()
    })

    it('non-keepOpen items call action and close', () => {
      const action = vi.fn()
      const onClose = vi.fn()
      const item = { label: 'Copy', action }

      if (!item.disabled && item.action) {
        item.action()
      }
      if (!item.keepOpen) {
        onClose()
      }

      expect(action).toHaveBeenCalled()
      expect(onClose).toHaveBeenCalled()
    })
  })
})
