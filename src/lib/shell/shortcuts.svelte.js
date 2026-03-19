export function setupSearchShortcut({
  doc = document,
  onToggleSearch,
}) {
  const handler = (event) => {
    if (event.key === 'k' && (event.metaKey || event.ctrlKey)) {
      event.preventDefault()
      onToggleSearch?.()
    }
  }

  doc.addEventListener('keydown', handler)
  return () => doc.removeEventListener('keydown', handler)
}

export function setupHistoryNavigation({
  doc = document,
  onGoBack,
  onGoForward,
}) {
  function onMouseDown(event) {
    if (event.button === 3) {
      event.preventDefault()
      onGoBack?.()
    } else if (event.button === 4) {
      event.preventDefault()
      onGoForward?.()
    }
  }

  function onKeyDown(event) {
    if (event.altKey && event.key === 'ArrowLeft') {
      event.preventDefault()
      onGoBack?.()
    } else if (event.altKey && event.key === 'ArrowRight') {
      event.preventDefault()
      onGoForward?.()
    }
  }

  doc.addEventListener('mousedown', onMouseDown)
  doc.addEventListener('keydown', onKeyDown)

  return () => {
    doc.removeEventListener('mousedown', onMouseDown)
    doc.removeEventListener('keydown', onKeyDown)
  }
}
