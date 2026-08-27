export function createToolRegistryState(initialTools) {
  let currentTools = $state(initialTools)

  return {
    get tools() {
      return currentTools
    },
    set tools(value) {
      currentTools = value
    },
  }
}
