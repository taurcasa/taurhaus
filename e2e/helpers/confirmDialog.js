import { fastClick } from './navigation.js'

export async function isConfirmDialogOpen() {
  return await browser.execute(() =>
    document.querySelector('dialog[open][data-testid="confirm-dialog"]') !== null)
}

export async function clickOpenConfirmDialog() {
  const selector = 'dialog[open][data-testid="confirm-dialog"] [data-testid="confirm-dialog-confirm"]'
  const confirm = await $(selector)
  if (!(await confirm.isExisting()) || !(await confirm.isEnabled())) {
    throw new Error('Open confirmation action was unavailable')
  }
  const clicked = await fastClick(selector)
  if (!clicked) throw new Error('Open confirmation action was unavailable')
}
