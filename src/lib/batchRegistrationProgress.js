export function getBatchProgressName(payload) {
  if (!payload || typeof payload !== 'object') return ''
  return payload.projectName || payload.project_name || ''
}
