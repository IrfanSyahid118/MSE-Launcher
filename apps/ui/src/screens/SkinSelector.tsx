import { useEffect, useMemo, useRef, useState } from 'react'
import type { ChangeEvent, ReactNode } from 'react'
import {
  Check,
  ChevronDown,
  ChevronUp,
  Download,
  Info,
  Move,
  Package,
  Pencil,
  Plus,
  RefreshCw,
  Shield,
  ShieldOff,
  X,
} from 'lucide-react'
import { useI18n } from '../i18n'
import {
  useActiveAccount,
  useInstances,
  useInstallSkinResourcePack,
  useSetActiveCape,
  useSyncMinecraftProfile,
  useUpdateAccountSkin,
} from '../api/queries'
import { errorMessage } from '../api/launcher'
import { useAppStore } from '../stores/app-store'
import { MinecraftSkin3D } from '../components/MinecraftSkin/MinecraftSkin3D'
import { SkinThumbnail } from '../components/MinecraftSkin/SkinThumbnail'
import { resolveSkinUrl, useActiveSkin } from '../components/MinecraftSkin/useActiveSkin'
import { extractCapeFaces, getAlexSkinDataUrl, getSteveSkinDataUrl } from '../components/MinecraftSkin/skin-textures'
import { AccountDialog } from '../components/AccountDialog/AccountDialog'
import { GlassDialog } from '../components/GlassDialog/GlassDialog'
import { RightSidebar } from '../components/RightSidebar/RightSidebar'
import styles from './SkinSelector.module.css'

interface SkinEntry {
  id: string
  name: string
  skinUrl: string
}

/** Model lengan: klasik 4 px atau slim 3 px. */
type ArmModel = 'classic' | 'slim'

const DEFAULT_STEVE: SkinEntry = { id: 'steve', name: 'Steve', skinUrl: getSteveSkinDataUrl() }
const DEFAULT_ALEX: SkinEntry = { id: 'alex', name: 'Alex', skinUrl: getAlexSkinDataUrl() }
const DEFAULT_SKINS: SkinEntry[] = [DEFAULT_STEVE, DEFAULT_ALEX]

function getCapeDisplayName(alias: string | undefined, fallbackLabel: string): string {
  if (!alias) return fallbackLabel
  switch (alias.toLowerCase()) {
    case 'home':
      return 'Vanilla Cape'
    case 'copper':
      return '15th Anniversary'
    case 'menace':
      return 'TikTok Cape'
    case 'pan':
      return 'Pancake Cape'
    case 'common':
      return 'Twitch Cape'
    default:
      return alias
  }
}

function CapeThumbnail({ capeUrl, name }: { capeUrl?: string | undefined; name: string }): ReactNode {
  const [faceUrl, setFaceUrl] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    if (!capeUrl) {
      setFaceUrl(null)
      return
    }
    void extractCapeFaces(capeUrl).then((faces) => {
      if (active && faces) {
        setFaceUrl(faces.back)
      }
    })
    return () => {
      active = false
    }
  }, [capeUrl])

  if (faceUrl) {
    return (
      <img
        src={faceUrl}
        alt={name}
        style={{
          width: 30,
          height: 46,
          imageRendering: 'pixelated',
          borderRadius: 3,
          boxShadow: '0 2px 6px rgba(0,0,0,0.3)',
        }}
      />
    )
  }

  return <Shield size={24} style={{ opacity: 0.6 }} />
}

export function SkinSelector(): ReactNode {
  const { t } = useI18n()
  const { account } = useActiveAccount()
  const activeSkin = useActiveSkin(account)
  const instancesQuery = useInstances()
  const installResourcePack = useInstallSkinResourcePack()
  const updateAccountSkin = useUpdateAccountSkin()
  const syncProfile = useSyncMinecraftProfile()
  const setActiveCape = useSetActiveCape()

  const customSkinUrl = useAppStore((state) => state.customSkinUrl)
  const accountSkinType = useAppStore((state) =>
    account ? state.accountSkinTypes[account.id] : undefined,
  )
  const accountCustomSkin = useAppStore((state) =>
    account ? state.customSkinsByAccount[account.id] : undefined,
  )
  const setSelectedSkin = useAppStore((state) => state.setSelectedSkin)
  const setSelectedSkinType = useAppStore((state) => state.setSelectedSkinType)
  const setAccountSkinType = useAppStore((state) => state.setAccountSkinType)
  const setAccountCustomSkin = useAppStore((state) => state.setAccountCustomSkin)
  const setSkinSlim = useAppStore((state) => state.setSkinSlim)
  const setCustomSkinUrl = useAppStore((state) => state.setCustomSkinUrl)
  const pushToast = useAppStore((state) => state.pushToast)

  const [selection, setSelection] = useState<SkinEntry>({
    id: activeSkin.skinType,
    name: t('skin.currentSkin'),
    skinUrl: activeSkin.skinUrl,
  })
  const [armModel, setArmModel] = useState<ArmModel>(activeSkin.isSlim ? 'slim' : 'classic')
  const [viewBack, setViewBack] = useState(false)
  const [savedSkinsOpen, setSavedSkinsOpen] = useState(true)
  const [capesOpen, setCapesOpen] = useState(true)
  const [defaultSkinsOpen, setDefaultSkinsOpen] = useState(true)
  const [accountDialogOpen, setAccountDialogOpen] = useState(false)
  const [editDialogOpen, setEditDialogOpen] = useState(false)
  const [packDialogOpen, setPackDialogOpen] = useState(false)
  const [targetInstanceId, setTargetInstanceId] = useState<string>('all')

  const fileInputRef = useRef<HTMLInputElement>(null)
  const capeFileInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!account) return

    if (accountSkinType === 'steve') {
      setArmModel('classic')
      setSkinSlim(false)
      setSelection(DEFAULT_STEVE)
      setSelectedSkin('steve')
      setSelectedSkinType('steve')
      return
    }
    if (accountSkinType === 'alex') {
      setArmModel('slim')
      setSkinSlim(true)
      setSelection(DEFAULT_ALEX)
      setSelectedSkin('alex')
      setSelectedSkinType('alex')
      return
    }

    const isAccountSlim = account.skinVariant === 'slim'
    setArmModel(isAccountSlim ? 'slim' : 'classic')
    setSkinSlim(isAccountSlim)

    if (account.skinUrl) {
      if (account.skinUrl === DEFAULT_ALEX.skinUrl) {
        setSelection(DEFAULT_ALEX)
        setSelectedSkin('alex')
        setSelectedSkinType('alex')
      } else if (account.skinUrl === DEFAULT_STEVE.skinUrl) {
        setSelection(DEFAULT_STEVE)
        setSelectedSkin('steve')
        setSelectedSkinType('steve')
      } else {
        const resolvedUrl = resolveSkinUrl(account.skinUrl) ?? account.skinUrl
        const isCustomMicrosoft = account.type === 'microsoft' && accountSkinType === 'custom'
        const resolvedType = account.type === 'microsoft' && !isCustomMicrosoft ? 'microsoft' : 'custom'
        setSelection({
          id: resolvedType,
          name: resolvedType === 'microsoft' ? `${account.username} (${t('settings.mojangBadge')})` : t('skin.customSkin'),
          skinUrl: resolvedUrl,
        })
        setSelectedSkinType(resolvedType)
        if (resolvedType === 'custom') {
          setAccountCustomSkin(account.id, resolvedUrl)
        }
      }
    }
  }, [
    account,
    accountSkinType,
    setAccountCustomSkin,
    setSkinSlim,
    setSelectedSkin,
    setSelectedSkinType,
    t,
  ])

  const isSlim = armModel === 'slim'

  const activeCustomSkin = account ? accountCustomSkin : customSkinUrl
  const savedSkins = useMemo<SkinEntry[]>(
    () => (activeCustomSkin ? [{ id: 'custom', name: t('skin.uploadedSkin'), skinUrl: activeCustomSkin }] : []),
    [activeCustomSkin, t],
  )

  const applySkin = async (entry: SkinEntry): Promise<void> => {
    let nextArmModel: ArmModel = armModel
    if (entry.id === 'alex') {
      nextArmModel = 'slim'
    } else if (entry.id === 'steve') {
      nextArmModel = 'classic'
    }

    if (account) {
      const isDisplayUrl =
        entry.skinUrl.startsWith('asset:') ||
        entry.skinUrl.startsWith('http://asset.localhost') ||
        entry.skinUrl.startsWith('https://asset.localhost')
      const fallbackPresetUrl = nextArmModel === 'slim' ? DEFAULT_ALEX.skinUrl : DEFAULT_STEVE.skinUrl
      const skinData = isDisplayUrl ? (account.skinUrl ?? fallbackPresetUrl) : entry.skinUrl
      try {
        await updateAccountSkin.mutateAsync({
          accountId: account.id,
          skinData,
          skinVariant: nextArmModel === 'slim' ? 'slim' : 'classic',
          capeData: account.capeUrl,
        })
      } catch (err) {
        pushToast(
          t('skin.updateFailedToast', { error: errorMessage(err, t('common.unknown')) }),
          'error',
        )
        return
      }
    }

    setSelection(entry)
    if (entry.id === 'alex') {
      setArmModel('slim')
      setSkinSlim(true)
      setSelectedSkin('alex')
      setSelectedSkinType('alex')
      if (account) setAccountSkinType(account.id, 'alex')
    } else if (entry.id === 'steve') {
      setArmModel('classic')
      setSkinSlim(false)
      setSelectedSkin('steve')
      setSelectedSkinType('steve')
      if (account) setAccountSkinType(account.id, 'steve')
    } else {
      setSelectedSkinType('custom')
      if (account) {
        setAccountCustomSkin(account.id, entry.skinUrl)
        setAccountSkinType(account.id, 'custom')
      }
    }

    pushToast(
      t('skin.appliedToast', {
        name: entry.name,
        arm: nextArmModel === 'slim' ? t('skin.armSlimShort') : t('skin.armClassicShort'),
      }),
    )
  }

  const applyArmModel = async (next: ArmModel): Promise<void> => {
    setArmModel(next)
    setSkinSlim(next === 'slim')
    if (account) {
      const isPreset = selection.id === 'alex' || selection.id === 'steve'
      const rawCustom =
        customSkinUrl &&
        !customSkinUrl.startsWith('asset:') &&
        !customSkinUrl.startsWith('http://asset.localhost') &&
        !customSkinUrl.startsWith('https://asset.localhost')
          ? customSkinUrl
          : undefined
      const fallbackPresetUrl = next === 'slim' ? DEFAULT_ALEX.skinUrl : DEFAULT_STEVE.skinUrl
      const skinData = isPreset
        ? selection.skinUrl
        : (account.skinUrl ?? rawCustom ?? fallbackPresetUrl)
      try {
        await updateAccountSkin.mutateAsync({
          accountId: account.id,
          skinData,
          skinVariant: next === 'slim' ? 'slim' : 'classic',
          capeData: account.capeUrl,
        })
      } catch (err) {
        pushToast(
          t('skin.updateFailedToast', { error: errorMessage(err, t('common.unknown')) }),
          'error',
        )
      }
    }
  }

  const handleFileUpload = (event: ChangeEvent<HTMLInputElement>): void => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!file.name.toLowerCase().endsWith('.png')) {
      pushToast(t('skin.pngOnlyError'), 'error')
      return
    }
    const reader = new FileReader()
    reader.onload = async (loadEvent) => {
      const dataUrl = loadEvent.target?.result
      if (typeof dataUrl !== 'string') {
        pushToast(t('skin.readFileError'), 'error')
        return
      }
      if (account) {
        try {
          await updateAccountSkin.mutateAsync({
            accountId: account.id,
            skinData: dataUrl,
            skinVariant: isSlim ? 'slim' : 'classic',
            capeData: account.capeUrl,
          })
        } catch (err) {
          pushToast(
            t('skin.updateFailedToast', { error: errorMessage(err, t('common.unknown')) }),
            'error',
          )
          return
        }
      }
      if (account) {
        setAccountCustomSkin(account.id, dataUrl)
        setAccountSkinType(account.id, 'custom')
      } else {
        setCustomSkinUrl(dataUrl)
      }
      setSelectedSkinType('custom')
      setSelection({ id: 'custom', name: t('skin.uploadedSkin'), skinUrl: dataUrl })
      pushToast(t('skin.addedSavedToast'))
    }
    reader.onerror = () => {
      pushToast(t('skin.readFileError'), 'error')
    }
    reader.readAsDataURL(file)
  }

  const handleSyncMojang = async (): Promise<void> => {
    if (!account) return
    try {
      const updated = await syncProfile.mutateAsync({ accountId: account.id })
      setAccountCustomSkin(account.id, null)
      setAccountSkinType(account.id, 'microsoft')
      setSelectedSkinType('microsoft')
      if (updated.skinUrl) {
        const resolvedUrl = resolveSkinUrl(updated.skinUrl) ?? updated.skinUrl
        setSelection({
          id: 'microsoft',
          name: `${updated.username} (${t('settings.mojangBadge')})`,
          skinUrl: resolvedUrl,
        })
      }
      if (updated.skinVariant) {
        setArmModel(updated.skinVariant === 'slim' ? 'slim' : 'classic')
        setSkinSlim(updated.skinVariant === 'slim')
      }
      const capeCount = updated.capes?.length ?? 0
      pushToast(
        t('skin.syncSuccessToast', { username: updated.username, count: capeCount }),
      )
    } catch (err) {
      pushToast(t('skin.syncFailedToast', { error: String(err) }), 'error')
    }
  }

  const handleSelectCape = async (capeId: string | undefined): Promise<void> => {
    if (!account) {
      pushToast(t('skin.selectAccountFirst'), 'error')
      return
    }

    try {
      await setActiveCape.mutateAsync({
        accountId: account.id,
        capeId,
      })
      setViewBack(true)
      if (capeId) {
        const found = account.capes?.find((c) => c.id === capeId)
        const name = getCapeDisplayName(found?.alias, t('skin.officialCape'))
        pushToast(t('skin.capeActivatedToast', { name }))
      } else {
        pushToast(t('skin.capeRemovedToast'))
      }
    } catch (err) {
      pushToast(t('skin.capeChangeFailedToast', { error: String(err) }), 'error')
    }
  }

  const handleCapeUpload = (event: ChangeEvent<HTMLInputElement>): void => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!file.name.toLowerCase().endsWith('.png')) {
      pushToast(t('skin.capePngOnlyError'), 'error')
      return
    }
    if (!account) {
      pushToast(t('skin.selectAccountFirst'), 'error')
      return
    }
    const reader = new FileReader()
    reader.onload = async (loadEvent) => {
      const dataUrl = loadEvent.target?.result
      if (typeof dataUrl !== 'string') return
      try {
        await setActiveCape.mutateAsync({
          accountId: account.id,
          customCapeData: dataUrl,
        })
        setViewBack(true)
        pushToast(t('skin.customCapeInstalledToast'))
      } catch (err) {
        pushToast(t('skin.customCapeFailedToast', { error: String(err) }), 'error')
      }
    }
    reader.onerror = () => {
      pushToast(t('skin.readFileError'), 'error')
    }
    reader.readAsDataURL(file)
  }

  const handleInstallPack = async (): Promise<void> => {
    if (!account) {
      pushToast(t('skin.selectOrCreateAccountFirst'), 'error')
      return
    }

    try {
      const instanceIds =
        targetInstanceId === 'all'
          ? []
          : [targetInstanceId]

      const res = await installResourcePack.mutateAsync({
        accountId: account.id,
        instanceIds,
      })

      pushToast(
        t('skin.packInstalledToast', {
          file: res.packFileName,
          count: res.updatedInstances,
        }),
      )
      setPackDialogOpen(false)
    } catch (err) {
      pushToast(t('skin.packInstallFailedToast', { error: String(err) }), 'error')
    }
  }

  const requiresSignIn = account?.type !== 'microsoft'

  return (
    <div className={styles.page} data-testid="skin-selector-screen">
      <div className={styles.container}>
        <div className={styles.mainLayout}>
          <div className={styles.previewColumn}>
            <div className={styles.modelBox}>
              {!editDialogOpen ? (
                <MinecraftSkin3D
                  skinUrl={selection.skinUrl}
                  capeUrl={activeSkin.capeUrl}
                  isSlim={isSlim}
                  interactive
                  bare
                  zoom={1.32}
                  viewBack={viewBack}
                  className={styles.model}
                />
              ) : null}
            </div>

            <div className={styles.viewToggleRow} role="group" aria-label={t('skin.viewAngleAria')}>
              <button
                type="button"
                className={`${styles.viewToggleButton} ${!viewBack ? styles.viewToggleButtonActive : ''}`}
                onClick={() => setViewBack(false)}
              >
                {t('skin.viewFront')}
              </button>
              <button
                type="button"
                className={`${styles.viewToggleButton} ${viewBack ? styles.viewToggleButtonActive : ''}`}
                onClick={() => setViewBack(true)}
              >
                {t('skin.viewBack')}
              </button>
            </div>

            <p className={styles.dragHint}>
              <Move size={13} aria-hidden />
              <span>{t('skin.dragToRotate')}</span>
            </p>

            <div className={styles.armRow}>
              <span className={styles.armLabel}>{t('skin.armModelLabel')}</span>
              <div className={styles.armToggle} role="radiogroup" aria-label={t('skin.armModelLabel')}>
                <button
                  type="button"
                  role="radio"
                  aria-checked={armModel === 'classic'}
                  className={`${styles.armOption} ${armModel === 'classic' ? styles.armOptionActive : ''}`}
                  onClick={() => applyArmModel('classic')}
                  data-testid="skin-model-classic"
                >
                  {t('skin.armClassic')}
                </button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={armModel === 'slim'}
                  className={`${styles.armOption} ${armModel === 'slim' ? styles.armOptionActive : ''}`}
                  onClick={() => applyArmModel('slim')}
                  data-testid="skin-model-slim"
                >
                  {t('skin.armSlim')}
                </button>
              </div>
              <span className={styles.armHint}>
                {isSlim ? t('skin.armHintSlim') : t('skin.armHintClassic')}
              </span>
            </div>

            {activeSkin.capeUrl ? (
              <div className={styles.armRow}>
                <span className={styles.armLabel}>{t('skin.capeLabel')}</span>
                <span className={styles.armHint} style={{ color: 'var(--color-primary, #10b981)', fontWeight: 500 }}>
                  {t('skin.capeStatusActive', {
                    source:
                      account?.type === 'microsoft'
                        ? t('skin.capeSourceOfficial')
                        : t('skin.capeSourceCustom'),
                  })}
                </span>
              </div>
            ) : null}

            {account?.type === 'microsoft' ? (
              <button
                type="button"
                className={styles.syncMojangButton}
                onClick={handleSyncMojang}
                disabled={syncProfile.isPending}
                title={t('skin.syncMojangTitle')}
              >
                <RefreshCw size={13} className={syncProfile.isPending ? styles.spin : ''} aria-hidden />
                <span>{syncProfile.isPending ? t('skin.syncingMojang') : t('skin.syncMojang')}</span>
              </button>
            ) : null}

            <div className={styles.buttonRow}>
              <button
                type="button"
                className={styles.editSkinButton}
                onClick={() => setEditDialogOpen(true)}
              >
                <Pencil size={13} aria-hidden />
                <span>{t('skin.editSkin')}</span>
              </button>
              <button
                type="button"
                className={styles.installPackButton}
                onClick={() => setPackDialogOpen(true)}
                title={t('skin.installResourcePackTitle')}
              >
                <Package size={13} aria-hidden />
                <span>{t('skin.installResourcePack')}</span>
              </button>
            </div>
          </div>

          <div className={styles.skinsColumn}>
            {/* Jubah (Capes) Section */}
            <section className={styles.section}>
              <button
                type="button"
                className={styles.sectionHeader}
                onClick={() => setCapesOpen((prev) => !prev)}
                aria-expanded={capesOpen}
              >
                <span className={styles.sectionTitle}>
                  {t('skin.capesSection')}{' '}
                  {account?.capes && account.capes.length > 0
                    ? t('skin.officialCount', { count: account.capes.length })
                    : ''}
                </span>
                {capesOpen ? (
                  <ChevronUp size={15} aria-hidden />
                ) : (
                  <ChevronDown size={15} aria-hidden />
                )}
              </button>

              {capesOpen ? (
                <div className={styles.capeGrid}>
                  {/* Tanpa Jubah (No Cape) */}
                  <button
                    type="button"
                    className={`${styles.capeCard} ${!activeSkin.capeUrl ? styles.capeCardActive : ''}`}
                    onClick={() => void handleSelectCape(undefined)}
                    title={t('skin.noCape')}
                  >
                    {!activeSkin.capeUrl ? (
                      <span className={styles.checkBadge} aria-hidden>
                        <Check size={11} strokeWidth={3} />
                      </span>
                    ) : null}
                    <div className={styles.capeFigure}>
                      <ShieldOff size={24} style={{ opacity: 0.6 }} />
                    </div>
                    <span className={styles.capeName}>{t('skin.noCape')}</span>
                  </button>

                  {/* Official Mojang Capes */}
                  {account?.capes?.map((cape) => {
                    const isCapeActive = account.activeCapeId === cape.id || (cape.state === 'ACTIVE' && activeSkin.capeUrl)
                    const displayName = getCapeDisplayName(cape.alias, t('skin.officialCape'))
                    return (
                      <button
                        key={cape.id}
                        type="button"
                        className={`${styles.capeCard} ${isCapeActive ? styles.capeCardActive : ''}`}
                        onClick={() => void handleSelectCape(cape.id)}
                        title={displayName}
                      >
                        {isCapeActive ? (
                          <span className={styles.checkBadge} aria-hidden>
                            <Check size={11} strokeWidth={3} />
                          </span>
                        ) : null}
                        <div className={styles.capeFigure}>
                          <CapeThumbnail capeUrl={cape.url} name={displayName} />
                        </div>
                        <span className={styles.capeName}>{displayName}</span>
                      </button>
                    )
                  })}

                  {/* Unggah Jubah Kustom */}
                  <button
                    type="button"
                    className={styles.capeCard}
                    onClick={() => capeFileInputRef.current?.click()}
                    style={{ borderStyle: 'dashed' }}
                    title={t('skin.uploadCustomCapeTitle')}
                  >
                    <Plus size={20} className={styles.addIcon} aria-hidden />
                    <span className={styles.addTitle} style={{ fontSize: 11.5 }}>{t('skin.customCape')}</span>
                    <span className={styles.addSubtitle}>PNG 64×32</span>
                  </button>
                </div>
              ) : null}
            </section>

            <section className={styles.section}>
              <button
                type="button"
                className={styles.sectionHeader}
                onClick={() => setSavedSkinsOpen((prev) => !prev)}
                aria-expanded={savedSkinsOpen}
              >
                <span className={styles.sectionTitle}>{t('skin.savedSkins')}</span>
                {savedSkinsOpen ? (
                  <ChevronUp size={15} aria-hidden />
                ) : (
                  <ChevronDown size={15} aria-hidden />
                )}
              </button>

              {savedSkinsOpen ? (
                <div className={styles.savedGrid}>
                  <button
                    type="button"
                    className={styles.addSkinCard}
                    onClick={() => fileInputRef.current?.click()}
                  >
                    <Plus size={20} className={styles.addIcon} aria-hidden />
                    <span className={styles.addTitle}>{t('skin.addSkin')}</span>
                    <span className={styles.addSubtitle}>PNG 64×64</span>
                  </button>

                  {savedSkins.map((skin) => (
                    <SkinCard
                      key={skin.id}
                      skin={skin}
                      isSlim={isSlim}
                      selected={selection.id === skin.id}
                      onSelect={() => applySkin(skin)}
                    />
                  ))}
                </div>
              ) : null}
            </section>

            <section className={styles.section}>
              <button
                type="button"
                className={styles.sectionHeader}
                onClick={() => setDefaultSkinsOpen((prev) => !prev)}
                aria-expanded={defaultSkinsOpen}
              >
                <span className={styles.sectionTitle}>{t('skin.defaultSkins')}</span>
                {defaultSkinsOpen ? (
                  <ChevronUp size={15} aria-hidden />
                ) : (
                  <ChevronDown size={15} aria-hidden />
                )}
              </button>

              {defaultSkinsOpen ? (
                <div className={styles.defaultGrid}>
                  {DEFAULT_SKINS.map((skin) => (
                    <SkinCard
                      key={skin.id}
                      skin={skin}
                      isSlim={isSlim}
                      selected={selection.id === skin.id}
                      onSelect={() => applySkin(skin)}
                    />
                  ))}
                </div>
              ) : null}
            </section>

            <p className={styles.gameNote}>
              {requiresSignIn ? t('skin.offlineGameNote') : t('skin.onlineGameNote')}
            </p>
          </div>
        </div>

        <RightSidebar mode="home" />
      </div>

      {requiresSignIn ? (
        <div className={styles.demoBanner}>
          <div className={styles.bannerInfo}>
            <Info size={16} className={styles.infoIcon} aria-hidden />
            <div className={styles.bannerText}>
              <span className={styles.bannerTitle}>{t('skin.offlineBannerTitle')}</span>
              <span className={styles.bannerDesc}>{t('skin.offlineBannerDesc')}</span>
            </div>
          </div>

          <button
            type="button"
            className={styles.microsoftButton}
            onClick={() => setAccountDialogOpen(true)}
          >
            <svg viewBox="0 0 20 20" width="13" height="13" fill="currentColor" aria-hidden>
              <rect x="2" y="2" width="7.5" height="7.5" />
              <rect x="10.5" y="2" width="7.5" height="7.5" />
              <rect x="2" y="10.5" width="7.5" height="7.5" />
              <rect x="10.5" y="10.5" width="7.5" height="7.5" />
            </svg>
            <span>{t('account.microsoftSignIn')}</span>
          </button>
        </div>
      ) : null}

      <input
        type="file"
        ref={fileInputRef}
        onChange={handleFileUpload}
        accept="image/png"
        style={{ display: 'none' }}
        aria-hidden
        tabIndex={-1}
      />

      <input
        type="file"
        ref={capeFileInputRef}
        onChange={handleCapeUpload}
        accept="image/png"
        style={{ display: 'none' }}
        aria-hidden
        tabIndex={-1}
      />

      <GlassDialog
        open={editDialogOpen}
        onOpenChange={setEditDialogOpen}
        title={t('skin.editingSkinTitle')}
        closeLabel={t('common.close')}
        size="lg"
        footer={
          <div className={styles.dialogActions}>
            <button
              type="button"
              className={styles.secondaryButton}
              onClick={() => setEditDialogOpen(false)}
            >
              <X size={13} aria-hidden />
              <span>{t('common.cancel')}</span>
            </button>
            <button
              type="button"
              className={styles.primaryButton}
              onClick={() => {
                setEditDialogOpen(false)
                setSkinSlim(armModel === 'slim')
                pushToast(t('skin.savedForLauncherToast'))
              }}
            >
              <Check size={13} aria-hidden />
              <span>{t('skin.done')}</span>
            </button>
          </div>
        }
      >
        <div className={styles.editLayout}>
          <div className={styles.editPreview}>
            <MinecraftSkin3D
              skinUrl={selection.skinUrl}
              capeUrl={activeSkin.capeUrl}
              isSlim={isSlim}
              interactive
              bare
              zoom={1.32}
              viewBack={viewBack}
              className={styles.model}
            />
            <p className={styles.dragHint}>
              <Move size={13} aria-hidden />
              <span>{t('skin.dragToRotate')}</span>
            </p>
          </div>

          <div className={styles.capeColumn}>
            <span className={styles.capeLabel}>{t('skin.armModelLabel')}</span>
            <div className={styles.armToggle} role="radiogroup" aria-label={t('skin.armModelLabel')}>
              <button
                type="button"
                role="radio"
                aria-checked={armModel === 'classic'}
                className={`${styles.armOption} ${armModel === 'classic' ? styles.armOptionActive : ''}`}
                onClick={() => applyArmModel('classic')}
                data-testid="skin-dialog-model-classic"
              >
                {t('skin.armClassic')}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={armModel === 'slim'}
                className={`${styles.armOption} ${armModel === 'slim' ? styles.armOptionActive : ''}`}
                onClick={() => applyArmModel('slim')}
                data-testid="skin-dialog-model-slim"
              >
                {t('skin.armSlim')}
              </button>
            </div>
            <p className={styles.capeHint}>{t('skin.editArmHint')}</p>
          </div>
        </div>
      </GlassDialog>

      <GlassDialog
        open={packDialogOpen}
        onOpenChange={setPackDialogOpen}
        title={t('skin.installPackTitle')}
        closeLabel={t('common.close')}
        size="md"
        footer={
          <div className={styles.dialogActions}>
            <button
              type="button"
              className={styles.secondaryButton}
              onClick={() => setPackDialogOpen(false)}
            >
              <X size={13} aria-hidden />
              <span>{t('common.cancel')}</span>
            </button>
            <button
              type="button"
              className={styles.primaryButton}
              onClick={handleInstallPack}
              disabled={installResourcePack.isPending}
            >
              <Download size={13} aria-hidden />
              <span>{installResourcePack.isPending ? t('browse.installing') : t('skin.installToGame')}</span>
            </button>
          </div>
        }
      >
        <div className={styles.dialogForm}>
          <p className={styles.dialogInfo}>{t('skin.packDialogInfo')}</p>

          <label className={styles.dialogLabel} htmlFor="target-instance-select">
            {t('skin.selectTargetInstance')}
          </label>
          <select
            id="target-instance-select"
            className={styles.dialogSelect}
            value={targetInstanceId}
            onChange={(e) => setTargetInstanceId(e.target.value)}
          >
            <option value="all">{t('skin.allInstancesOption')}</option>
            {instancesQuery.data?.map((inst) => (
              <option key={inst.id} value={inst.id}>
                {inst.name} ({inst.loader.type} {inst.minecraftVersion})
              </option>
            ))}
          </select>
        </div>
      </GlassDialog>

      <AccountDialog open={accountDialogOpen} onOpenChange={setAccountDialogOpen} />
    </div>
  )
}

interface SkinCardProps {
  skin: SkinEntry
  isSlim: boolean
  selected: boolean
  onSelect: () => void
}

function SkinCard({ skin, isSlim, selected, onSelect }: SkinCardProps): ReactNode {
  return (
    <button
      type="button"
      className={`${styles.skinCard} ${selected ? styles.skinCardActive : ''}`}
      onClick={onSelect}
      aria-pressed={selected}
      title={skin.name}
    >
      {selected ? (
        <span className={styles.checkBadge} aria-hidden>
          <Check size={11} strokeWidth={3} />
        </span>
      ) : null}
      <span className={styles.skinFigure}>
        <SkinThumbnail skinUrl={skin.skinUrl} isSlim={isSlim} />
      </span>
      <span className={styles.skinName}>{skin.name}</span>
    </button>
  )
}
