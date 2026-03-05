import { describe, it, expect } from 'vitest'
import { normalizeProjectPath, resolveRelativePath } from './pathUtils.js'

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

describe('normalizeProjectPath', () => {
  it('normalizes WSL UNC paths to linux paths', () => {
    expect(normalizeProjectPath('\\\\wsl$\\Ubuntu\\home\\user\\proj\\')).toBe('/home/user/proj')
    expect(normalizeProjectPath('\\\\wsl.localhost\\Ubuntu\\home\\user\\proj\\')).toBe('/home/user/proj')
  })

  it('normalizes Windows drive paths to /mnt form', () => {
    expect(normalizeProjectPath('D:\\projects\\taurhaus\\')).toBe('/mnt/d/projects/taurhaus')
    expect(normalizeProjectPath('c:/Users/me/code')).toBe('/mnt/c/Users/me/code')
  })

  it('normalizes native and relative paths', () => {
    expect(normalizeProjectPath('/home/user//proj///')).toBe('/home/user/proj')
    expect(normalizeProjectPath('foo\\bar///baz/')).toBe('foo/bar/baz')
  })

  it('returns root and empty values safely', () => {
    expect(normalizeProjectPath('/')).toBe('/')
    expect(normalizeProjectPath('')).toBe('')
    expect(normalizeProjectPath('   ')).toBe('')
  })
})
