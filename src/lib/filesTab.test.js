import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  getFileTree: vi.fn(),
  readFile: vi.fn(),
  readProjectAsset: vi.fn(),
}))

vi.mock('./CodeViewer.svelte', () => ({
  default: function MockCodeViewer(target, props) {
    const el = document.createElement('div')
    el.setAttribute('data-testid', 'mock-code-viewer')
    el.textContent = props?.code || ''
    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(el)
    } else {
      target.parentNode.insertBefore(el, target)
    }
    return {
      $set(nextProps) {
        el.textContent = nextProps?.code || ''
      },
      $destroy() {
        el.remove()
      },
    }
  },
}))

vi.mock('./MarkdownRenderer.svelte', () => ({
  default: function MockMarkdownRenderer(target, props) {
    const el = document.createElement('div')
    el.setAttribute('data-testid', 'mock-markdown-renderer')
    el.textContent = props?.source || ''
    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(el)
    } else {
      target.parentNode.insertBefore(el, target)
    }
    return {
      $set(nextProps) {
        el.textContent = nextProps?.source || ''
      },
      $destroy() {
        el.remove()
      },
    }
  },
}))

const { getFileTree, readFile, readProjectAsset } = await import('./ipc.js')

import FilesTab from './FilesTab.svelte'

function createDeferred() {
  /** @type {(value: any) => void} */
  let resolve
  /** @type {(reason?: any) => void} */
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('FilesTab', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getFileTree.mockResolvedValue([
      { path: 'src/first.js', name: 'first.js', is_dir: false },
      { path: 'src/second.js', name: 'second.js', is_dir: false },
    ])
    readProjectAsset.mockResolvedValue(null)
  })

  it('ignores stale file responses when switching files quickly', async () => {
    const firstRead = createDeferred()
    const secondRead = createDeferred()
    readFile.mockImplementation((_, path) => {
      if (path === 'src/first.js') return firstRead.promise
      if (path === 'src/second.js') return secondRead.promise
      return Promise.resolve({ content: '', language: 'js' })
    })

    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'first.js' })).toBeInTheDocument()
      expect(screen.getByRole('treeitem', { name: 'second.js' })).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'first.js' }))
    await fireEvent.click(screen.getByRole('treeitem', { name: 'second.js' }))

    secondRead.resolve({ content: 'second-content', language: 'javascript' })
    await waitFor(() => {
      expect(screen.getByTestId('mock-code-viewer')).toHaveTextContent('second-content')
    })

    firstRead.resolve({ content: 'first-content', language: 'javascript' })
    await waitFor(() => {
      expect(screen.getByTestId('mock-code-viewer')).toHaveTextContent('second-content')
      expect(screen.getByTestId('mock-code-viewer')).not.toHaveTextContent('first-content')
    })
  })

  it('virtualizes large file trees to viewport-sized DOM nodes', async () => {
    getFileTree.mockResolvedValue(
      Array.from({ length: 220 }, (_, index) => ({
        path: `src/file-${String(index).padStart(3, '0')}.js`,
        name: `file-${String(index).padStart(3, '0')}.js`,
        is_dir: false,
      }))
    )
    readFile.mockResolvedValue({ content: '', language: 'javascript' })

    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'file-000.js' })).toBeInTheDocument()
    })

    const renderedNodes = screen.getAllByTestId('file-tree-node')
    expect(renderedNodes.length).toBeLessThan(120)
  })

  it('updates virtualized file tree window on scroll', async () => {
    getFileTree.mockResolvedValue(
      Array.from({ length: 220 }, (_, index) => ({
        path: `src/file-${String(index).padStart(3, '0')}.js`,
        name: `file-${String(index).padStart(3, '0')}.js`,
        is_dir: false,
      }))
    )
    readFile.mockResolvedValue({ content: '', language: 'javascript' })

    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'file-000.js' })).toBeInTheDocument()
    })
    expect(screen.queryByRole('treeitem', { name: 'file-180.js' })).not.toBeInTheDocument()

    const scroller = screen.getByTestId('file-tree-scroll')
    scroller.scrollTop = 32 * 170
    await fireEvent.scroll(scroller)

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'file-180.js' })).toBeInTheDocument()
    })
  })
})
