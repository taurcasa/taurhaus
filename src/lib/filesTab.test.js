import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import * as assetCache from './assetCache.js'

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
    assetCache.clear()
    if (!navigator.clipboard) {
      Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
    } else {
      navigator.clipboard.writeText = vi.fn().mockResolvedValue(undefined)
    }
    getFileTree.mockResolvedValue([
      { path: 'src/first.js', name: 'first.js', is_dir: false },
      { path: 'src/second.js', name: 'second.js', is_dir: false },
    ])
    readFile.mockResolvedValue({ content: '', language: 'javascript' })
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

  it('keeps first file read result when selectedProject metadata updates with same project id', async () => {
    const firstRead = createDeferred()
    readFile.mockImplementation((_, path) => {
      if (path === 'src/first.js') return firstRead.promise
      return Promise.resolve({ content: '', language: 'javascript' })
    })

    const { rerender } = render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-2', path: '/tmp/project-2', branch: 'main', is_dirty: false },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'first.js' })).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'first.js' }))
    await waitFor(() => {
      expect(screen.getByTestId('filecontent-loading')).toBeInTheDocument()
    })

    // Simulates Shell replacing selectedProject with updated git metadata.
    await rerender({
      dark: false,
      codeTheme: 'github-light',
      selectedProject: { id: 'project-2', path: '/tmp/project-2', branch: 'feature/x', is_dirty: true },
      isActive: true,
    })

    firstRead.resolve({ content: 'first-project-content', language: 'javascript' })

    await waitFor(() => {
      expect(screen.getByTestId('mock-code-viewer')).toHaveTextContent('first-project-content')
    })
  })

  it('clears loading when a project switch invalidates an in-flight file request', async () => {
    const firstRead = createDeferred()
    getFileTree.mockImplementation((projectId) => {
      if (projectId === 'project-2') {
        return Promise.resolve([
          { path: 'src/first.js', name: 'first.js', is_dir: false },
          { path: 'src/second.js', name: 'second.js', is_dir: false },
        ])
      }
      return Promise.resolve([
        { path: 'src/first.js', name: 'first.js', is_dir: false },
        { path: 'src/second.js', name: 'second.js', is_dir: false },
      ])
    })
    readFile.mockImplementation((_, path) => {
      if (path === 'src/first.js') return firstRead.promise
      return Promise.resolve({ content: '', language: 'javascript' })
    })

    const { rerender } = render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'first.js' })).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'first.js' }))
    await waitFor(() => {
      expect(screen.getByTestId('filecontent-loading')).toBeInTheDocument()
    })

    await rerender({
      dark: false,
      codeTheme: 'github-light',
      selectedProject: { id: 'project-2', path: '/tmp/project-2' },
      isActive: true,
    })

    firstRead.resolve({ content: 'first-content', language: 'javascript' })

    await waitFor(() => {
      expect(screen.queryByTestId('filecontent-loading')).not.toBeInTheDocument()
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

  it('shows empty-tree message when no files are returned', async () => {
    getFileTree.mockResolvedValue([])
    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByText('No viewable files')).toBeInTheDocument()
    })
  })

  it('auto-selects README on initial tree load', async () => {
    getFileTree.mockResolvedValue([
      { path: 'src', name: 'src', is_dir: true, children: [{ path: 'src/main.js', name: 'main.js', is_dir: false }] },
      { path: 'README.md', name: 'README.md', is_dir: false },
    ])
    readFile.mockResolvedValue({ content: '# Hello', language: 'markdown' })

    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByText('README.md')).toBeInTheDocument()
      expect(readFile).toHaveBeenCalledWith('project-1', 'README.md')
    })
  })

  it('renders image via IPC when cache misses and sets error when image load fails', async () => {
    getFileTree.mockResolvedValue([
      { path: 'images/photo.png', name: 'photo.png', is_dir: false },
      { path: 'images/missing.png', name: 'missing.png', is_dir: false },
    ])
    readProjectAsset
      .mockResolvedValueOnce('data:image/png;base64,AAAA')
      .mockResolvedValueOnce(null)

    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'photo.png' })).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'photo.png' }))
    await waitFor(() => {
      expect(screen.getByRole('img', { name: 'images/photo.png' })).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'missing.png' }))
    await waitFor(() => {
      expect(screen.getByText('Error loading file')).toBeInTheDocument()
    })
  })

  it('renders binary, pdf, and too-large error states for non-text paths', async () => {
    getFileTree.mockResolvedValue([
      { path: 'bin/model.bin', name: 'model.bin', is_dir: false },
      { path: 'docs/report.pdf', name: 'report.pdf', is_dir: false },
      { path: 'src/huge.txt', name: 'huge.txt', is_dir: false },
    ])
    readFile.mockRejectedValue(new Error('file too large for preview'))

    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'model.bin' })).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'model.bin' }))
    await waitFor(() => {
      expect(screen.getByText('Binary file — cannot display as text')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'report.pdf' }))
    await waitFor(() => {
      expect(screen.getByText('PDF viewer coming soon')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('treeitem', { name: 'huge.txt' }))
    await waitFor(() => {
      expect(screen.getByText('File too large to display (>5 MB)')).toBeInTheDocument()
    })
  })

  it('copies both absolute and relative paths from file context menu', async () => {
    getFileTree.mockResolvedValue([
      { path: '/tmp/project-1/src/main.js', name: 'main.js', is_dir: false },
    ])
    readFile.mockResolvedValue({ content: 'console.log(1)', language: 'javascript' })

    render(FilesTab, {
      props: {
        dark: false,
        codeTheme: 'github-light',
        selectedProject: { id: 'project-1', path: '/tmp/project-1' },
        isActive: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: 'main.js' })).toBeInTheDocument()
    })

    await fireEvent.contextMenu(screen.getByRole('treeitem', { name: 'main.js' }))
    await fireEvent.mouseDown(screen.getByText('Copy Path'))
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('/tmp/project-1/src/main.js')

    await fireEvent.contextMenu(screen.getByRole('treeitem', { name: 'main.js' }))
    await fireEvent.mouseDown(screen.getByText('Copy Relative Path'))
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('src/main.js')
  })
})
