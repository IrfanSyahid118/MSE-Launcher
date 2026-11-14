import type { Account } from '@mc/shared'
import { useAppStore } from '../../stores/app-store'
import type { SkinModel, SkinType } from '../../stores/app-store'
import { getAlexSkinDataUrl, getSteveSkinDataUrl } from './skin-textures'

export interface ActiveSkinInfo {
  skinUrl: string
  isSlim: boolean
  skinType: SkinType
  model: SkinModel
  isMicrosoft: boolean
  hasCustom: boolean
  capeUrl?: string | undefined
}

export function resolveSkinUrl(pathOrUrl: string | undefined | null): string | undefined {
  if (!pathOrUrl) return undefined
  if (
    pathOrUrl.startsWith('data:') ||
    pathOrUrl.startsWith('blob:')
  ) {
    return pathOrUrl
  }
  if (pathOrUrl.startsWith('http://') || pathOrUrl.startsWith('https://')) {
    if (pathOrUrl.startsWith('http://textures.minecraft.net/')) {
      return pathOrUrl.replace('http://', 'https://')
    }
    return pathOrUrl
  }
  const tauriInternals = (
    window as unknown as {
      __TAURI_INTERNALS__?: {
        convertFileSrc?: (path: string, protocol?: string) => string
      }
    }
  ).__TAURI_INTERNALS__

  if (typeof tauriInternals?.convertFileSrc === 'function') {
    return tauriInternals.convertFileSrc(pathOrUrl, 'asset')
  }
  return pathOrUrl
}

/**
 * Tekstur skin aktif dan model lengannya.
 *
 * Model (`isSlim`) datang dari preferensi terpisah, jadi tekstur yang sama
 * bisa dipakai sebagai karakter klasik maupun slim. Urutan sumber tekstur:
 * skin Microsoft resmi, skin custom yang diunggah / tersimpan di akun, lalu Steve/Alex.
 */
export function useActiveSkin(account: Account | null | undefined): ActiveSkinInfo {
  const selectedSkinType = useAppStore((state) => state.selectedSkinType)
  const skinSlim = useAppStore((state) => state.skinSlim)
  const customSkinUrl = useAppStore((state) => state.customSkinUrl)

  const isMicrosoftAccount = account?.type === 'microsoft' && Boolean(account.uuid)
  const isSlim = account?.skinVariant ? account.skinVariant === 'slim' : skinSlim

  const rawSkin = customSkinUrl || account?.skinUrl
  const resolvedSkin = resolveSkinUrl(rawSkin)
  const hasCustom = Boolean(resolvedSkin)
  const capeUrl = resolveSkinUrl(account?.capeUrl)

  if (isMicrosoftAccount && (selectedSkinType === 'microsoft' || !customSkinUrl)) {
    const skinUrl = resolvedSkin || `https://crafatar.com/skins/${account.uuid}`
    return {
      skinUrl,
      isSlim,
      skinType: 'microsoft',
      model: isSlim ? 'alex' : 'steve',
      isMicrosoft: true,
      hasCustom,
      capeUrl,
    }
  }

  if (resolvedSkin && (selectedSkinType === 'custom' || (!customSkinUrl && account?.skinUrl))) {
    return {
      skinUrl: resolvedSkin,
      isSlim,
      skinType: 'custom',
      model: isSlim ? 'alex' : 'steve',
      isMicrosoft: isMicrosoftAccount,
      hasCustom: true,
      capeUrl,
    }
  }

  if (selectedSkinType === 'alex') {
    return {
      skinUrl: getAlexSkinDataUrl(),
      isSlim,
      skinType: 'alex',
      model: 'alex',
      isMicrosoft: isMicrosoftAccount,
      hasCustom,
      capeUrl,
    }
  }

  return {
    skinUrl: getSteveSkinDataUrl(),
    isSlim,
    skinType: 'steve',
    model: 'steve',
    isMicrosoft: isMicrosoftAccount,
    hasCustom,
    capeUrl,
  }
}
