/*
 * TODO: Replace with Tauri IPC commands (invoke) once the backend exists.
 *
 * This is hardcoded mock data for UI development. Every export in this file
 * must be replaced with real data from the Rust backend before shipping.
 * The data shapes here are approximate — the real schema gets defined in
 * Phase 4 (Architecture). Do not build abstractions around these shapes.
 */

export const projects = [
  { name: 'taurhaus', branch: 'main', status: 'active', dirty: false },
  { name: 'missing_invoice_reloaded', branch: 'feat/auth', status: 'active', dirty: true },
  { name: 'taurui', branch: 'main', status: 'active', dirty: false },
  { name: 'taursec', branch: 'main', status: 'recent', dirty: false },
  { name: 'taursult', branch: 'main', status: 'recent', dirty: false },
  { name: 'ledger', branch: 'main', status: 'stale', dirty: false },
  { name: 'aitx', branch: 'main', status: 'stale', dirty: false },
  { name: 'taurmolt', branch: 'main', status: 'dormant', dirty: false },
  { name: 'taurora', branch: 'develop', status: 'dormant', dirty: false },
  { name: 'taurox', branch: 'main', status: 'dormant', dirty: false },
]

export const selectedProject = {
  name: 'taurhaus',
  branch: 'main',
  status: 'active',
  dirty: false,
  description: 'Desktop tool for AI project management',
  path: '~/projects/taurhaus',
  tags: ['tauri-app', 'svelte', 'design'],
}

export const latestSession = {
  date: '2026-02-16',
  timeAgo: '2 hours ago',
  summary: 'Completed Phase 3 UI Design cohesion review. Found and fixed 9 cross-document issues including search query preservation, divergence indicators, relationship modal spec, sidebar breakpoint contradictions, and search result category alignment.',
  nextSteps: [
    'Build visual prototypes — three design proposals for main layout',
    'Compare proposals and select visual direction',
    'Scaffold Tauri 2 project with chosen design',
    'Phase 4: Architecture — Rust backend modules and data models',
  ],
  openQuestions: [
    'Virtual scrolling library choice for large project lists',
    'Markdown renderer selection: marked vs unified/remark',
  ],
}

export const commits = [
  { hash: 'a1b2c3d', message: 'Fix Phase 3 cohesion issues across all docs', time: '2h' },
  { hash: 'e4f5g6h', message: 'Complete Phase 3G implementation spec', time: '5h' },
  { hash: 'i7j8k9l', message: 'Add phase-3f visual system', time: '1d' },
  { hash: 'm0n1o2p', message: 'Write phase-3e view designs', time: '1d' },
  { hash: 'q3r4s5t', message: 'Complete phase-3d information architecture', time: '2d' },
  { hash: 'u6v7w8x', message: 'Add phase-3c user journey maps', time: '3d' },
  { hash: 'y9z0a1b', message: 'Write phase-3b domain understanding', time: '4d' },
]

export const relationships = [
  { target: 'taurui', type: 'uses design from', direction: 'outgoing' },
  { target: 'taursec', type: 'audited by', direction: 'incoming' },
  { target: 'taursult', type: 'integrates with', direction: 'outgoing' },
]

export const sessionHistory = [
  { date: '2026-02-15', summary: 'Completed Phase 3F visual system — colors, typography, 18 components, motion.' },
  { date: '2026-02-14', summary: 'Wrote Phase 3E view designs for all six views plus registration modal.' },
  { date: '2026-02-13', summary: 'Finished Phase 3D information architecture — views, navigation, components.' },
  { date: '2026-02-12', summary: 'Completed Phase 3C user journey mapping — 9 journeys with priority scoring.' },
]

export const groups = [
  { key: 'active', label: 'ACTIVE' },
  { key: 'recent', label: 'RECENT' },
  { key: 'stale', label: 'STALE' },
  { key: 'dormant', label: 'DORMANT' },
]
