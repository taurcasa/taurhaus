export async function syncWindowsStartupViewport({
  attempted,
  isTauriRuntime,
  getPlatform,
  loadWindowApi = () => import('@tauri-apps/api/window'),
  requestNextFrame = () => new Promise((resolve) => requestAnimationFrame(resolve)),
  dispatchResize = () => window.dispatchEvent(new Event('resize')),
  logger = console,
}) {
  if (attempted || !isTauriRuntime) return

  try {
    const platform = await getPlatform()
    if (platform !== 'windows') return

    const { getCurrentWindow, PhysicalSize } = await loadWindowApi()
    const appWindow = getCurrentWindow()
    const [maximized, fullscreen] = await Promise.all([
      appWindow.isMaximized(),
      appWindow.isFullscreen(),
    ])

    if (maximized || fullscreen) {
      dispatchResize()
      return
    }

    await requestNextFrame()
    const size = await appWindow.innerSize()
    if (!size?.width || !size?.height) return

    await appWindow.setSize(new PhysicalSize(size.width + 1, size.height))
    await appWindow.setSize(new PhysicalSize(size.width, size.height))
    dispatchResize()
  } catch (error) {
    logger.warn('[window] startup viewport sync failed:', error)
  }
}

async function withWindow(action, loadWindowApi = () => import('@tauri-apps/api/window')) {
  try {
    const { getCurrentWindow } = await loadWindowApi()
    await action(getCurrentWindow())
  } catch {
    // Ignore dev mode where the Tauri runtime is not available.
  }
}

export function minimizeShellWindow(loadWindowApi) {
  return withWindow((windowHandle) => windowHandle.minimize(), loadWindowApi)
}

export function toggleShellMaximize(loadWindowApi) {
  return withWindow((windowHandle) => windowHandle.toggleMaximize(), loadWindowApi)
}

export function closeShellWindow(loadWindowApi) {
  return withWindow((windowHandle) => windowHandle.close(), loadWindowApi)
}
