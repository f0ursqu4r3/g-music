export const themes = ['midnight', 'plum', 'ember'] as const

export type ThemeName = (typeof themes)[number]

export function readTheme(value: string | null): ThemeName {
  return themes.includes(value as ThemeName) ? (value as ThemeName) : 'midnight'
}

/** Other WebViews with the same origin receive storage changes. */
export function useTheme() {
  let stored: string | null = null
  try {
    stored = window.localStorage.getItem('gmusic-theme')
  } catch {
    /* Use the default when storage is unavailable. */
  }
  const theme = ref<ThemeName>(readTheme(stored))
  watch(
    theme,
    (value) => {
      document.documentElement.dataset.theme = value
      try {
        window.localStorage.setItem('gmusic-theme', value)
      } catch {
        /* Apply the theme in this window. */
      }
    },
    { immediate: true },
  )
  function receive(event: StorageEvent) {
    if (event.key === 'gmusic-theme' || event.key === null) theme.value = readTheme(event.newValue)
  }
  onMounted(() => window.addEventListener('storage', receive))
  onUnmounted(() => window.removeEventListener('storage', receive))
  return theme
}
import { onMounted, onUnmounted, ref, watch } from 'vue'
