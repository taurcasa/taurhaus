export function createStateBridge(definitions) {
  const bridge = {}

  for (const [key, [getValue, setValue]] of Object.entries(definitions)) {
    Object.defineProperty(bridge, key, {
      enumerable: true,
      configurable: true,
      get: getValue,
      set: setValue,
    })
  }

  return bridge
}
