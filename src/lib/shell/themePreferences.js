export async function loadThemePreferences({ getSettings, defaultLightTheme, defaultDarkTheme }) {
  const settings = await getSettings()
  const codeTheme = settings?.code_theme
  return {
    codeThemeLight: codeTheme?.light || defaultLightTheme,
    codeThemeDark: codeTheme?.dark || defaultDarkTheme,
    darkMode: !!settings?.dark_mode,
  }
}

export async function persistDarkModePreference({ getSettings, updateSettings, value }) {
  const settings = await getSettings()
  await updateSettings({ ...settings, dark_mode: value })
}
