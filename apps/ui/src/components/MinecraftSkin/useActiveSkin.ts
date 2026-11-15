import type { ChangeEvent } from 'react'
import type { Account } from '@mc/shared'
import { useI18n } from '../../i18n'
import { errorMessage } from '../../api/launcher'
import { useUpdateAccountSkin } from '../../api/queries'
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
    pathOrUrl.startsWith('blob:') ||
    pathOrUrl.startsWith('asset:')
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
 * Prioritas mengikuti pilihan per-akun (jika ada), lalu data akun tersimpan
 * (Microsoft -> Crafatar/Mojang, Offline + upload -> custom, Offline -> Steve/Alex).
 */
export function useActiveSkin(account: Account | null | undefined): ActiveSkinInfo {
  const selectedSkin = useAppStore((state) => state.selectedSkin)
  const selectedSkinType = useAppStore((state) => state.selectedSkinType)
  const accountSkinType = useAppStore((state) =>
    account ? state.accountSkinTypes[account.id] : undefined,
  )
  const accountCustomSkin = useAppStore((state) =>
    account ? state.customSkinsByAccount[account.id] : undefined,
  )
  const skinSlim = useAppStore((state) => state.skinSlim)
  const customSkinUrl = useAppStore((state) => state.customSkinUrl)

  const steveUrl = getSteveSkinDataUrl()
  const alexUrl = getAlexSkinDataUrl()

  if (!account) {
    const resolvedGuestCustom = resolveSkinUrl(customSkinUrl)
    if (selectedSkinType === 'custom' && resolvedGuestCustom) {
      return {
        skinUrl: resolvedGuestCustom,
        isSlim: skinSlim,
        skinType: 'custom',
        model: skinSlim ? 'alex' : 'steve',
        isMicrosoft: false,
        hasCustom: true,
        capeUrl: undefined,
      }
    }
    const guestAlex =
      selectedSkinType === 'alex' || (selectedSkin === 'alex' && selectedSkinType !== 'steve')
    return {
      skinUrl: guestAlex ? alexUrl : steveUrl,
      isSlim: guestAlex,
      skinType: guestAlex ? 'alex' : 'steve',
      model: guestAlex ? 'alex' : 'steve',
      isMicrosoft: false,
      hasCustom: Boolean(resolvedGuestCustom),
      capeUrl: undefined,
    }
  }

  const isMicrosoftAccount = account.type === 'microsoft' && Boolean(account.uuid)
  const resolvedAccountSkin = resolveSkinUrl(account.skinUrl)
  const resolvedPerAccountCustom = resolveSkinUrl(accountCustomSkin)
  const capeUrl = resolveSkinUrl(account.capeUrl)

  const isPresetSteve = account.skinUrl === steveUrl
  const isPresetAlex = account.skinUrl === alexUrl
  const hasPersistedCustomSkin = Boolean(resolvedAccountSkin && !isPresetSteve && !isPresetAlex)

  const customSkinSource =
    resolvedPerAccountCustom ?? (hasPersistedCustomSkin ? resolvedAccountSkin : undefined)

  const hasCustom = Boolean(
    resolvedPerAccountCustom ||
      (accountSkinType === 'custom' && hasPersistedCustomSkin) ||
      (!isMicrosoftAccount && hasPersistedCustomSkin),
  )

  let effectiveType: SkinType
  if (accountSkinType === 'microsoft' && isMicrosoftAccount) {
    effectiveType = 'microsoft'
  } else if (accountSkinType === 'steve') {
    effectiveType = 'steve'
  } else if (accountSkinType === 'alex') {
    effectiveType = 'alex'
  } else if (accountSkinType === 'custom' && customSkinSource) {
    effectiveType = 'custom'
  } else if (isPresetSteve) {
    effectiveType = 'steve'
  } else if (isPresetAlex) {
    effectiveType = 'alex'
  } else if (isMicrosoftAccount) {
    effectiveType = 'microsoft'
  } else if (hasPersistedCustomSkin) {
    effectiveType = 'custom'
  } else if (account.skinVariant === 'slim') {
    effectiveType = 'alex'
  } else {
    effectiveType = 'steve'
  }

  const isSlim =
    effectiveType === 'alex'
      ? true
      : effectiveType === 'steve'
        ? false
        : account.skinVariant
          ? account.skinVariant === 'slim'
          : skinSlim

  if (effectiveType === 'steve') {
    return {
      skinUrl: steveUrl,
      isSlim: false,
      skinType: 'steve',
      model: 'steve',
      isMicrosoft: isMicrosoftAccount,
      hasCustom,
      capeUrl,
    }
  }

  if (effectiveType === 'alex') {
    return {
      skinUrl: alexUrl,
      isSlim: true,
      skinType: 'alex',
      model: 'alex',
      isMicrosoft: isMicrosoftAccount,
      hasCustom,
      capeUrl,
    }
  }

  if (effectiveType === 'custom' && customSkinSource) {
    return {
      skinUrl: customSkinSource,
      isSlim,
      skinType: 'custom',
      model: isSlim ? 'alex' : 'steve',
      isMicrosoft: isMicrosoftAccount,
      hasCustom: true,
      capeUrl,
    }
  }

  if (isMicrosoftAccount) {
    const cleanUuid = account.uuid.replace(/-/g, '').trim()
    return {
      skinUrl: resolvedAccountSkin || `https://mc-heads.net/skin/${encodeURIComponent(cleanUuid)}`,
      isSlim,
      skinType: 'microsoft',
      model: isSlim ? 'alex' : 'steve',
      isMicrosoft: true,
      hasCustom,
      capeUrl,
    }
  }

  return {
    skinUrl: isSlim ? alexUrl : steveUrl,
    isSlim,
    skinType: isSlim ? 'alex' : 'steve',
    model: isSlim ? 'alex' : 'steve',
    isMicrosoft: false,
    hasCustom,
    capeUrl,
  }
}

/** Memilih tipe skin untuk akun tertentu (atau tamu bila belum masuk). */
export function useSelectSkinType(account: Account | null | undefined): (type: SkinType) => void {
  const setSelectedSkin = useAppStore((state) => state.setSelectedSkin)
  const setSelectedSkinType = useAppStore((state) => state.setSelectedSkinType)
  const setAccountSkinType = useAppStore((state) => state.setAccountSkinType)
  const setSkinSlim = useAppStore((state) => state.setSkinSlim)

  return (type: SkinType) => {
    if (type === 'steve') {
      setSelectedSkin('steve')
      setSkinSlim(false)
    } else if (type === 'alex') {
      setSelectedSkin('alex')
      setSkinSlim(true)
    }
    if (account) {
      setAccountSkinType(account.id, type)
    }
    setSelectedSkinType(type)
  }
}

/** Mengunggah file skin PNG untuk akun aktif (atau tamu) dan menyimpannya ke profil akun. */
export function useUploadAccountSkin(
  account: Account | null | undefined,
  isSlim: boolean,
): (event: ChangeEvent<HTMLInputElement>) => void {
  const { t } = useI18n()
  const updateAccountSkin = useUpdateAccountSkin()
  const pushToast = useAppStore((state) => state.pushToast)
  const setSelectedSkinType = useAppStore((state) => state.setSelectedSkinType)
  const setAccountSkinType = useAppStore((state) => state.setAccountSkinType)
  const setAccountCustomSkin = useAppStore((state) => state.setAccountCustomSkin)
  const setCustomSkinUrl = useAppStore((state) => state.setCustomSkinUrl)

  return (e: ChangeEvent<HTMLInputElement>): void => {
    const file = e.target.files?.[0]
    e.target.value = ''
    if (!file) return
    const reader = new FileReader()
    reader.onload = (event) => {
      const dataUrl = event.target?.result
      if (typeof dataUrl !== 'string') return
      if (account) {
        setAccountCustomSkin(account.id, dataUrl)
        setAccountSkinType(account.id, 'custom')
        setSelectedSkinType('custom')
        updateAccountSkin.mutate(
          {
            accountId: account.id,
            skinData: dataUrl,
            skinVariant: isSlim ? 'slim' : 'classic',
            capeData: account.capeUrl,
          },
          {
            onSuccess: () => {
              pushToast(t('account.skin.loaded'))
            },
            onError: (error) => {
              pushToast(errorMessage(error, t('error.unknown')), 'error')
            },
          },
        )
      } else {
        setCustomSkinUrl(dataUrl)
        setSelectedSkinType('custom')
        pushToast(t('account.skin.loaded'))
      }
    }
    reader.readAsDataURL(file)
  }
}
