const STARTUP_FAILURE_ID = 'startup-failure-overlay'
const STARTUP_FALLBACK_MESSAGE = 'Taurhaus failed to start. Check logs for details.'

export function extractStartupErrorMessage(error) {
  if (error && typeof error === 'object' && typeof error.message === 'string' && error.message.trim()) {
    return error.message
  }
  if (typeof error === 'string' && error.trim()) {
    return error
  }
  return STARTUP_FALLBACK_MESSAGE
}

export function renderStartupFailure(error) {
  if (typeof document === 'undefined') return

  const message = extractStartupErrorMessage(error)
  const target = document.getElementById('app') ?? document.body
  if (!target) return

  let overlay = document.getElementById(STARTUP_FAILURE_ID)
  if (!overlay) {
    overlay = document.createElement('div')
    overlay.id = STARTUP_FAILURE_ID
    overlay.setAttribute('role', 'alert')
    overlay.style.cssText = [
      'min-height:100vh',
      'display:flex',
      'align-items:center',
      'justify-content:center',
      'background:#0a2323',
      'color:#f7fbfb',
      'font-family:Geist,system-ui,sans-serif',
      'padding:24px',
      'box-sizing:border-box',
    ].join(';')
    target.replaceChildren(overlay)
  }

  overlay.innerHTML = `
    <div style="max-width:520px;border:1px solid rgba(255,255,255,0.12);border-radius:16px;padding:20px 22px;background:rgba(255,255,255,0.04);box-shadow:0 18px 48px rgba(0,0,0,0.32)">
      <div style="font-size:18px;font-weight:600;letter-spacing:-0.02em;margin-bottom:8px">Startup failed</div>
      <div style="font-size:13px;line-height:1.45;color:rgba(247,251,251,0.82)">${message}</div>
      <div style="font-size:12px;line-height:1.45;color:rgba(247,251,251,0.52);margin-top:12px">Check the Windows Taurhaus log for the detailed error.</div>
    </div>
  `
}
