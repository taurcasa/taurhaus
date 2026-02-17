import { describe, it, expect } from 'vitest'
import { classifyFile, isImage } from './fileClassifier.js'

describe('classifyFile', () => {
  it('classifies image extensions', () => {
    expect(classifyFile('logo.png')).toBe('image')
    expect(classifyFile('photo.jpg')).toBe('image')
    expect(classifyFile('photo.jpeg')).toBe('image')
    expect(classifyFile('icon.svg')).toBe('image')
    expect(classifyFile('anim.gif')).toBe('image')
    expect(classifyFile('hero.webp')).toBe('image')
    expect(classifyFile('favicon.ico')).toBe('image')
    expect(classifyFile('map.bmp')).toBe('image')
  })

  it('classifies markdown extensions', () => {
    expect(classifyFile('README.md')).toBe('markdown')
    expect(classifyFile('docs/guide.markdown')).toBe('markdown')
  })

  it('classifies known binary extensions', () => {
    expect(classifyFile('model.glb')).toBe('binary')
    expect(classifyFile('app.wasm')).toBe('binary')
    expect(classifyFile('program.exe')).toBe('binary')
    expect(classifyFile('archive.zip')).toBe('binary')
    expect(classifyFile('archive.tar')).toBe('binary')
    expect(classifyFile('data.db')).toBe('binary')
    expect(classifyFile('data.sqlite')).toBe('binary')
    expect(classifyFile('font.woff2')).toBe('binary')
    expect(classifyFile('video.mp4')).toBe('binary')
    expect(classifyFile('lib.dll')).toBe('binary')
    expect(classifyFile('lib.so')).toBe('binary')
    expect(classifyFile('code.pyc')).toBe('binary')
  })

  it('classifies PDF separately', () => {
    expect(classifyFile('document.pdf')).toBe('pdf')
  })

  it('classifies text/code files', () => {
    expect(classifyFile('main.rs')).toBe('text')
    expect(classifyFile('index.js')).toBe('text')
    expect(classifyFile('app.ts')).toBe('text')
    expect(classifyFile('config.toml')).toBe('text')
    expect(classifyFile('data.json')).toBe('text')
    expect(classifyFile('style.css')).toBe('text')
    expect(classifyFile('shell.svelte')).toBe('text')
    expect(classifyFile('scene.ron')).toBe('text')
    expect(classifyFile('Cargo.lock')).toBe('text')
  })

  it('is case-insensitive', () => {
    expect(classifyFile('LOGO.PNG')).toBe('image')
    expect(classifyFile('README.MD')).toBe('markdown')
    expect(classifyFile('model.GLB')).toBe('binary')
  })

  it('handles paths with directories', () => {
    expect(classifyFile('src/assets/logo.png')).toBe('image')
    expect(classifyFile('web/static/branding/logo.jpg')).toBe('image')
    expect(classifyFile('docs/README.md')).toBe('markdown')
  })

  it('returns text for files without extension', () => {
    expect(classifyFile('Makefile')).toBe('text')
    expect(classifyFile('Dockerfile')).toBe('text')
    expect(classifyFile('.gitignore')).toBe('text')
  })

  it('returns text for empty or null input', () => {
    expect(classifyFile('')).toBe('text')
    expect(classifyFile(null)).toBe('text')
  })
})

describe('isImage', () => {
  it('returns true for image files', () => {
    expect(isImage('logo.png')).toBe(true)
    expect(isImage('photo.jpg')).toBe(true)
    expect(isImage('icon.svg')).toBe(true)
  })

  it('returns false for non-image files', () => {
    expect(isImage('main.rs')).toBe(false)
    expect(isImage('README.md')).toBe(false)
    expect(isImage('model.glb')).toBe(false)
  })
})
