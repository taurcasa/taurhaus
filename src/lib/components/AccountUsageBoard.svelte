<script>
  import ContextMenu from '../ContextMenu.svelte'
  import { accountState } from '../accounts.svelte.js'
  import { buildAccountMenuChildren } from '../accountMenu.js'
  import { tools } from '../toolRegistry.js'

  let {
    states = null,
    x = 0,
    y = 0,
    dark = true,
    onManage = () => {},
    onClose = () => {},
  } = $props()

  function stateFor(tool) {
    return states?.[tool] ?? accountState(tool)
  }

  const items = $derived.by(() => {
    const rows = []
    for (const descriptor of tools()) {
      const accounts = stateFor(descriptor.id)?.accounts ?? []
      if (!accounts.length) continue
      if (rows.length) rows.push({ separator: true })
      rows.push({ label: descriptor.label, disabled: true })
      rows.push(...buildAccountMenuChildren({ accounts }))
    }
    if (rows.length) rows.push({ separator: true })
    rows.push({ label: 'Manage accounts →', action: onManage })
    return rows
  })
</script>

<ContextMenu {items} {x} {y} {dark} {onClose} />
