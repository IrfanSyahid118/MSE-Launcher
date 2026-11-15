import type { ReactNode } from 'react'
import { useI18n } from '../../i18n'
import { getSteveSkinDataUrl } from './skin-textures'
import { SkinThumbnail } from './SkinThumbnail'
import styles from './SkinFallback.module.css'

export interface SkinFallbackProps {
  skinUrl?: string | undefined
  isSlim?: boolean | undefined
  label: string
}

/*
 * Pratinjau statis untuk perangkat atau driver yang tidak bisa membuat konteks
 * WebGL. Yang digambar adalah karakter (potret depan), bukan tekstur mentah,
 * supaya tampilannya tetap terbaca sebagai pemain Minecraft.
 */
export function SkinFallback({ skinUrl, isSlim = false, label }: SkinFallbackProps): ReactNode {
  const { t } = useI18n()
  return (
    <div className={styles.fallback} role="img" aria-label={label}>
      <div className={styles.figure}>
        <SkinThumbnail
          skinUrl={skinUrl && skinUrl.trim() !== '' ? skinUrl : getSteveSkinDataUrl()}
          isSlim={isSlim}
          pixel={10}
          className={styles.canvas}
        />
      </div>
      <p className={styles.note}>{t('skin.fallback2dNote')}</p>
    </div>
  )
}
