<script>
  /**
   * Modal skin for the shared account picker.
   *
   * The settled-launch contract remains in accounts.svelte.js. This wrapper
   * owns only the viewport overlay and open-dialog usage refresh cadence.
   */
  import AccountPicker from './AccountPicker.svelte'

  let {
    tool,
    accounts = [],
    projectName = '',
    defaultAccountId = null,
    degraded = false,
    reason = null,
    preselectedAccountId = null,
    dark = false,
    onConfirm = () => {},
    onCancel = () => {},
    onRequestUsage = () => {},
    onAddAccount = () => {},
    onManageAccounts = () => {},
  } = $props()

  const USAGE_POLL_MS = 30 * 1000
  $effect(() => {
    const timer = setInterval(() => onRequestUsage(), USAGE_POLL_MS)
    return () => clearInterval(timer)
  })
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-8"
  data-shell-overlay
  data-testid="account-chooser-overlay"
  onmousedown={(event) => { if (event.target === event.currentTarget) onCancel() }}
>
  <AccountPicker
    {tool}
    {accounts}
    {projectName}
    {defaultAccountId}
    {degraded}
    {reason}
    {preselectedAccountId}
    {dark}
    skin="modal"
    testId="account-chooser"
    {onConfirm}
    {onCancel}
    {onAddAccount}
    {onManageAccounts}
  />
</div>
