import { describe, expect, it } from 'vitest'
import { buildMemberActionMessage } from './meshTabRuntime.svelte.js'

// Regression: 76d1f88e dropped the blanket pane->terminal-session rewrite (it
// would have mangled backend wake reasons), taking the translation for the
// warnings members.rs really emits with it. These pin the two real wordings.
describe('member action warnings', () => {
  it('translates the not-reusable pane warning into operator language', () => {
    expect(
      buildMemberActionMessage('Resumed.', [
        "existing pane was not reusable for 'builder'; created a new pane",
      ])
    ).toBe(
      "Resumed. Notes: existing terminal session was not reusable for 'builder'; created a new terminal session"
    )
  })

  it('keeps a backend wake reason verbatim', () => {
    expect(
      buildMemberActionMessage('Resumed.', ['builder: onboarding wake failed: member pane is dead'])
    ).toBe('Resumed. Notes: builder: onboarding wake failed: member pane is dead')
  })
})
