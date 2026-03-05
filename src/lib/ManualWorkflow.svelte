<script>
  import DirectoryBrowser from './DirectoryBrowser.svelte'

  let {
    dark = false,
    t,
    inputBg = '',
    manualPath = '',
    validating = false,
    validationMessage = null,
    manualError = null,
    pathIsValid = false,
    registering = false,
    onManualPathInput = () => {},
    onManualPathBlur = () => {},
    onManualEnter = () => {},
    onManualAdd = () => {},
    onBackToScan = () => {},
    onManualDirectorySelect = () => {},
  } = $props()
</script>

<div>
  <label for="manual-path" class="text-[13px] {t.textSecondary} mb-1.5 block">Project path</label>
  <div class="relative">
    <input
      id="manual-path"
      type="text"
      placeholder="~/projects/my-project"
      value={manualPath}
      class="w-full px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono pr-8"
      oninput={(event) => onManualPathInput(event.currentTarget.value)}
      onkeydown={(event) => event.key === 'Enter' && onManualEnter()}
      onblur={onManualPathBlur}
      data-testid="manual-path-input"
    />
    {#if validating}
      <div class="absolute right-2.5 top-1/2 -translate-y-1/2">
        <div class="w-3.5 h-3.5 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
      </div>
    {:else if validationMessage?.type === 'success'}
      <div class="absolute right-2.5 top-1/2 -translate-y-1/2 text-success-500">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
      </div>
    {/if}
  </div>

  {#if manualError}
    <p class="text-[12px] text-danger-500 mt-1.5" data-testid="manual-error">{manualError}</p>
  {:else if validationMessage}
    <p class="text-[12px] mt-1.5 {validationMessage.type === 'error' ? 'text-danger-500' : validationMessage.type === 'warning' ? 'text-warning-500' : 'text-success-500'}" data-testid="validation-message">{validationMessage.text}</p>
  {/if}

  <div class="mt-3">
    <DirectoryBrowser {dark} selectedPath={manualPath} onSelect={onManualDirectorySelect} maxHeight="180px" />
  </div>

  <div class="flex items-center justify-between mt-3">
    <button
      class="text-[12px] {t.linkColor} transition-colors"
      onclick={onBackToScan}
    >Back to scan</button>
    <button
      class="px-3 py-1.5 rounded-md bg-brand-600 text-white text-[12px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
      onclick={onManualAdd}
      disabled={!pathIsValid || registering}
      data-testid="manual-add-button"
    >{registering ? 'Adding...' : 'Add project'}</button>
  </div>
</div>
