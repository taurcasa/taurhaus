import { describe, it, expect } from 'vitest'
import { resolveRelativePath } from './pathUtils.js'

describe('resolveRelativePath', () => {
  it('resolves parent directory reference', () => {
    expect(resolveRelativePath('docs/architecture/daemon-protocol.md', '../daemon-protocol.jpg'))
      .toBe('docs/daemon-protocol.jpg')
  })

  it('resolves sibling file reference', () => {
    expect(resolveRelativePath('docs/architecture/daemon-protocol.md', 'diagram.jpg'))
      .toBe('docs/architecture/diagram.jpg')
  })

  it('resolves multiple parent traversals', () => {
    expect(resolveRelativePath('docs/architecture/daemon-protocol.md', '../../ARCHITECTURE.md'))
      .toBe('ARCHITECTURE.md')
  })

  it('passes through when filePath is null', () => {
    expect(resolveRelativePath(null, 'docs/image.jpg'))
      .toBe('docs/image.jpg')
  })

  it('passes through absolute paths unchanged', () => {
    expect(resolveRelativePath('docs/foo.md', '/absolute/path.jpg'))
      .toBe('/absolute/path.jpg')
  })

  it('resolves dot segments', () => {
    expect(resolveRelativePath('docs/foo.md', './image.jpg'))
      .toBe('docs/image.jpg')
  })

  it('handles file at project root', () => {
    expect(resolveRelativePath('README.md', 'docs/image.jpg'))
      .toBe('docs/image.jpg')
  })

  it('handles deeply nested paths', () => {
    expect(resolveRelativePath('a/b/c/d.md', '../../img.jpg'))
      .toBe('a/img.jpg')
  })
})
