export function autoDismissNotice({
  value,
  timeoutMs,
  getTimer,
  setTimer,
  clearValue,
}) {
  const existingTimer = getTimer()
  if (existingTimer) clearTimeout(existingTimer)
  if (!value) return

  const timer = setTimeout(() => {
    clearValue()
  }, timeoutMs)
  setTimer(timer)

  return () => {
    const activeTimer = getTimer()
    if (activeTimer) clearTimeout(activeTimer)
  }
}
