import { useEffect } from 'react'
import type { Settings } from '@mc/shared'

/**
 * Menerapkan tema ke <html> dan mengikuti perubahan tema OS saat mode
 * `system`. nativeTheme di main process sudah diset dari settings, jadi
 * matchMedia di renderer otomatis mengikuti.
 */
export function useThemeEffect(settings: Settings | undefined): void {
  useEffect(() => {
    if (!settings) {
      return
    }
    const root = document.documentElement
    const media = window.matchMedia('(prefers-color-scheme: dark)')

    const apply = (): void => {
      const resolved =
        settings.theme.mode === 'system' ? (media.matches ? 'dark' : 'light') : settings.theme.mode
      root.dataset['theme'] = resolved
      root.dataset['transparency'] = settings.theme.reduceTransparency ? 'reduced' : 'full'
      root.dataset['motion'] = settings.theme.reduceMotion ? 'reduced' : 'full'
    }

    apply()

    if (settings.theme.mode !== 'system') {
      return
    }
    media.addEventListener('change', apply)
    return () => {
      media.removeEventListener('change', apply)
    }
  }, [settings])
}
