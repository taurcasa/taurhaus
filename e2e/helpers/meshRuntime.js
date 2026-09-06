import { clickUntil } from './clickUntil.js'

export async function clickRuntimeAddAgent(wait) {
  await clickUntil(async () => {
    // Regression: 275d42d6 retried this control even when it meant Resume.
    // MeshRuntimeBar labels it Add Agent only for an active team. Check and
    // click in one browser task so a re-render cannot change its meaning.
    await browser.execute(() => {
      const action = document.querySelector('[data-testid="mesh-runtime-primary-action"]')
      if (action?.textContent?.trim() === 'Add Agent' && !action.disabled) action.click()
    })
  }, 'mesh-add-agent-form', wait)
}

export async function unlockRuntimeAddAgent(wait) {
  // Regression: acd3c5aa3 lost a single unlock click in the full suite.
  // The target-first check prevents a retry from locking the fields again.
  await clickUntil('mesh-add-agent-unlock-toggle',
    async () => (await (await $('[data-testid="mesh-add-agent-tool-select"]')).getAttribute('disabled')) === null,
    wait
  )
}
