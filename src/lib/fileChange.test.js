import { describe, it, expect } from 'vitest'
import { pathWasChanged, anyPathMatches } from './fileChange.js'

describe('pathWasChanged', () => {
  it('matches Linux absolute path against relative file', () => {
    const paths = ['/home/user/project/README.md']
    expect(pathWasChanged(paths, 'README.md')).toBe(true)
  })

  it('matches nested relative path', () => {
    const paths = ['/home/user/project/src/lib.rs']
    expect(pathWasChanged(paths, 'src/lib.rs')).toBe(true)
  })

  it('matches Windows backslash paths', () => {
    const paths = ['\\\\wsl.localhost\\Ubuntu\\home\\user\\project\\README.md']
    expect(pathWasChanged(paths, 'README.md')).toBe(true)
  })

  it('matches nested Windows backslash paths', () => {
    const paths = ['\\\\wsl.localhost\\Ubuntu\\home\\user\\project\\src\\main.rs']
    expect(pathWasChanged(paths, 'src/main.rs')).toBe(true)
  })

  it('returns false when file not in paths', () => {
    const paths = ['/home/user/project/other.txt']
    expect(pathWasChanged(paths, 'README.md')).toBe(false)
  })

  it('returns false for empty paths', () => {
    expect(pathWasChanged([], 'README.md')).toBe(false)
    expect(pathWasChanged(null, 'README.md')).toBe(false)
  })

  it('returns false for empty relativePath', () => {
    expect(pathWasChanged(['/home/user/README.md'], '')).toBe(false)
    expect(pathWasChanged(['/home/user/README.md'], null)).toBe(false)
  })

  it('does not false-positive on partial filename match', () => {
    const paths = ['/home/user/project/NOT_README.md']
    expect(pathWasChanged(paths, 'README.md')).toBe(false)
  })

  it('matches among multiple paths', () => {
    const paths = [
      '/home/user/project/src/main.rs',
      '/home/user/project/README.md',
      '/home/user/project/Cargo.toml',
    ]
    expect(pathWasChanged(paths, 'README.md')).toBe(true)
    expect(pathWasChanged(paths, 'src/main.rs')).toBe(true)
    expect(pathWasChanged(paths, 'package.json')).toBe(false)
  })
})

describe('anyPathMatches', () => {
  it('matches regex against paths', () => {
    const paths = ['/home/user/project/README.md']
    expect(anyPathMatches(paths, /readme\.md$/i)).toBe(true)
  })

  it('returns false when no match', () => {
    const paths = ['/home/user/project/src/main.rs']
    expect(anyPathMatches(paths, /readme\.md$/i)).toBe(false)
  })

  it('returns false for empty paths', () => {
    expect(anyPathMatches([], /readme/i)).toBe(false)
    expect(anyPathMatches(null, /readme/i)).toBe(false)
  })
})
