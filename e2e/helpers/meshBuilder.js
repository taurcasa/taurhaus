import { clickTestId } from './navigation.js'
import { WAIT_SHORT } from './timing.js'

export async function setReactiveInputValue(testId, value) {
  return await browser.execute(({ testId, value }) => {
    const input = document.querySelector(`[data-testid="${testId}"]`)
    if (!(input instanceof HTMLInputElement)) return false
    const valueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
    valueSetter?.call(input, String(value ?? ''))
    input.dispatchEvent(new Event('input', { bubbles: true }))
    return true
  }, { testId, value })
}

export async function setInlineBuilderTeamName(value = '') {
  await clickTestId('mesh-builder-team-name-display')
  const teamNameInput = await $('[data-testid="mesh-builder-team-name-input"]')
  await browser.waitUntil(
    async () => await teamNameInput.isExisting(),
    { ...WAIT_SHORT, timeoutMsg: 'Inline team name input did not appear' }
  )

  const currentTeamName = String(await teamNameInput.getValue()).trim()
  const dispatched = await setReactiveInputValue(
    'mesh-builder-team-name-input',
    value || currentTeamName || 'e2e-mesh-team'
  )

  if (!dispatched) throw new Error('Inline team name input was unavailable')
}
