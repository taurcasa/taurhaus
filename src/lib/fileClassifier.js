/**
 * File classifier — determines how to load and render a file based on extension.
 *
 * Classification happens in the frontend BEFORE any IPC call, so we never
 * waste a roundtrip on files we know can't be displayed as text.
 *
 * See docs/file-rendering-pipeline.md for the full pipeline design.
 */

const IMAGE_EXTENSIONS = new Set([
  'png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico', 'bmp',
])

const MARKDOWN_EXTENSIONS = new Set([
  'md', 'markdown',
])

const KNOWN_BINARY_EXTENSIONS = new Set([
  // 3D models
  'glb', 'gltf', 'fbx', 'obj', 'stl',
  // Compiled / bytecode
  'wasm', 'exe', 'dll', 'so', 'dylib', 'o', 'a', 'class', 'pyc', 'pyo',
  // Archives
  'zip', 'tar', 'gz', 'bz2', 'xz', '7z', 'rar', 'zst',
  // Databases
  'db', 'sqlite', 'sqlite3',
  // Binary data
  'bin', 'dat', 'pack', 'idx',
  // Media (non-image)
  'mp3', 'mp4', 'wav', 'ogg', 'webm', 'avi', 'mkv', 'flac', 'aac',
  // Fonts
  'woff', 'woff2', 'ttf', 'otf', 'eot',
  // Documents (PDF is separate — future viewer)
  'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx',
])

const PDF_EXTENSIONS = new Set([
  'pdf',
])

/**
 * Classify a file by its extension.
 * @param {string} relativePath — file path (only extension matters)
 * @returns {'image' | 'markdown' | 'binary' | 'pdf' | 'text'}
 */
export function classifyFile(relativePath) {
  const ext = getExtension(relativePath)
  if (!ext) return 'text'

  if (IMAGE_EXTENSIONS.has(ext)) return 'image'
  if (MARKDOWN_EXTENSIONS.has(ext)) return 'markdown'
  if (PDF_EXTENSIONS.has(ext)) return 'pdf'
  if (KNOWN_BINARY_EXTENSIONS.has(ext)) return 'binary'
  return 'text'
}

/**
 * Check if a file is an image.
 * @param {string} relativePath
 * @returns {boolean}
 */
export function isImage(relativePath) {
  return IMAGE_EXTENSIONS.has(getExtension(relativePath))
}

function getExtension(path) {
  if (!path) return ''
  const dot = path.lastIndexOf('.')
  if (dot === -1 || dot === path.length - 1) return ''
  return path.slice(dot + 1).toLowerCase()
}
