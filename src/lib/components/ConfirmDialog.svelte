<script>
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    open = $bindable(false),
    title = 'Confirm action',
    message = '',
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    variant = 'danger',
    onConfirm = () => {},
    onCancel = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const panelTone = $derived(dark ? 'bg-zinc-900 border-zinc-700 text-zinc-100' : 'bg-white border-zinc-200 text-zinc-900')
  const neutralGhost = $derived(
    dark
      ? 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/70'
      : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'
  )
  const confirmTone = $derived(
    variant === 'default'
      ? 'bg-brand-600 text-white hover:bg-brand-700'
      : 'bg-danger-500 text-white hover:bg-danger-600'
  )

  let dialogElement = $state(null)

  function openDialogElement(dialog) {
    if (typeof dialog.showModal === 'function') {
      dialog.showModal()
      return
    }
    dialog.setAttribute('open', 'open')
  }

  function closeDialogElement(dialog) {
    if (typeof dialog.close === 'function') {
      dialog.close()
      return
    }
    dialog.removeAttribute('open')
  }

  function dismiss() {
    if (!open) return
    open = false
    onCancel()
  }

  function handleConfirm() {
    onConfirm()
    open = false
  }

  function handleCancelEvent(event) {
    event.preventDefault()
    dismiss()
  }

  function handleBackdropClick(event) {
    if (event.target === dialogElement) {
      dismiss()
    }
  }

  function handleKeydown(event) {
    if (event.key === 'Enter') {
      event.preventDefault()
      handleConfirm()
      return
    }

    if (event.key === 'Escape') {
      event.preventDefault()
      dismiss()
    }
  }

  $effect(() => {
    const dialog = dialogElement
    if (!dialog) return
    if (open) {
      if (!dialog.open) {
        openDialogElement(dialog)
      }
      return
    }
    if (dialog.open) {
      closeDialogElement(dialog)
    }
  })
</script>

<dialog
  bind:this={dialogElement}
  class="m-0 rounded-lg border p-0 shadow-xl backdrop:bg-transparent"
  oncancel={handleCancelEvent}
  onclick={handleBackdropClick}
  onkeydown={handleKeydown}
  data-testid="confirm-dialog"
>
  <div class="w-[min(360px,calc(100vw-2rem))] rounded-lg border p-5 animate-[meshfade_150ms_ease-out] {panelTone}">
    <h2 class="text-sm font-semibold {t.textPrimary}">{title}</h2>
    <p class="mt-2 text-xs leading-relaxed {t.textSecondary}">{message}</p>

    <div class="mt-4 flex items-center justify-end gap-2">
      <button
        class="rounded-md px-3 py-1.5 text-xs transition-colors {neutralGhost}"
        onclick={dismiss}
        data-testid="confirm-dialog-cancel"
      >
        {cancelLabel}
      </button>
      <button
        class="rounded-md px-3 py-1.5 text-xs font-medium transition-colors {confirmTone}"
        onclick={handleConfirm}
        data-testid="confirm-dialog-confirm"
      >
        {confirmLabel}
      </button>
    </div>
  </div>
</dialog>

<style>
  dialog {
    background: transparent;
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.5);
  }

  @keyframes meshfade {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }
</style>
