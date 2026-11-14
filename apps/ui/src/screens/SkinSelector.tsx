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
import {
  useActiveAccount,
  useInstances,
  useInstallSkinResourcePack,
  useSetActiveCape,
  useSyncMinecraftProfile,
  useUpdateAccountSkin,
} from '../api/queries'
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

function getCapeDisplayName(alias?: string): string {
  if (!alias) return 'Jubah Resmi'
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
  const { account } = useActiveAccount()
  const activeSkin = useActiveSkin(account)
  const instancesQuery = useInstances()
  const installResourcePack = useInstallSkinResourcePack()
  const updateAccountSkin = useUpdateAccountSkin()
  const syncProfile = useSyncMinecraftProfile()
  const setActiveCape = useSetActiveCape()

  const customSkinUrl = useAppStore((state) => state.customSkinUrl)
  const setSelectedSkin = useAppStore((state) => state.setSelectedSkin)
  const setSelectedSkinType = useAppStore((state) => state.setSelectedSkinType)
  const setSkinSlim = useAppStore((state) => state.setSkinSlim)
  const setCustomSkinUrl = useAppStore((state) => state.setCustomSkinUrl)
  const pushToast = useAppStore((state) => state.pushToast)

  const [selection, setSelection] = useState<SkinEntry>({
    id: activeSkin.skinType,
    name: 'Current skin',
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

  // Sinkronkan state lokal saat akun atau activeSkin berubah
  useEffect(() => {
    if (!account) return

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
        setSelection({
          id: account.type === 'microsoft' ? 'microsoft' : 'custom',
          name: account.type === 'microsoft' ? `${account.username} (Mojang)` : 'Custom skin',
          skinUrl: resolvedUrl,
        })
        setSelectedSkinType(account.type === 'microsoft' ? 'microsoft' : 'custom')
        if (account.type !== 'microsoft') {
          setCustomSkinUrl(resolvedUrl)
        }
      }
    }
  }, [account, setSkinSlim, setSelectedSkin, setSelectedSkinType, setCustomSkinUrl])

  const isSlim = armModel === 'slim'

  const savedSkins = useMemo<SkinEntry[]>(
    () => (customSkinUrl ? [{ id: 'custom', name: 'Uploaded skin', skinUrl: customSkinUrl }] : []),
    [customSkinUrl],
  )

  const applySkin = (entry: SkinEntry): void => {
    setSelection(entry)

    let nextArmModel: ArmModel = armModel
    if (entry.id === 'alex') {
      nextArmModel = 'slim'
      setArmModel('slim')
      setSkinSlim(true)
      setSelectedSkin('alex')
      setSelectedSkinType('alex')
    } else if (entry.id === 'steve') {
      nextArmModel = 'classic'
      setArmModel('classic')
      setSkinSlim(false)
      setSelectedSkin('steve')
      setSelectedSkinType('steve')
    } else {
      setSelectedSkinType('custom')
    }

    if (account) {
      updateAccountSkin.mutate({
        accountId: account.id,
        skinData: entry.skinUrl,
        skinVariant: nextArmModel === 'slim' ? 'slim' : 'classic',
        capeData: account.capeUrl,
      })
    }
    pushToast(`Skin "${entry.name}" dipakai (${nextArmModel === 'slim' ? 'Slim 3px' : 'Klasik 4px'}).`)
  }

  const applyArmModel = (next: ArmModel): void => {
    setArmModel(next)
    setSkinSlim(next === 'slim')
    if (account) {
      const skinData =
        selection.id === 'alex' || selection.id === 'steve'
          ? selection.skinUrl
          : (account.skinUrl ?? selection.skinUrl)
      updateAccountSkin.mutate({
        accountId: account.id,
        skinData,
        skinVariant: next === 'slim' ? 'slim' : 'classic',
        capeData: account.capeUrl,
      })
    }
  }

  const handleFileUpload = (event: ChangeEvent<HTMLInputElement>): void => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!file.name.toLowerCase().endsWith('.png')) {
      pushToast('Skin harus berupa berkas PNG.', 'error')
      return
    }
    const reader = new FileReader()
    reader.onload = (loadEvent) => {
      const dataUrl = loadEvent.target?.result
      if (typeof dataUrl !== 'string') return
      setCustomSkinUrl(dataUrl)
      setSelectedSkinType('custom')
      setSelection({ id: 'custom', name: 'Uploaded skin', skinUrl: dataUrl })
      if (account) {
        updateAccountSkin.mutate({
          accountId: account.id,
          skinData: dataUrl,
          skinVariant: isSlim ? 'slim' : 'classic',
          capeData: account.capeUrl,
        })
      }
      pushToast('Skin ditambahkan ke daftar tersimpan.')
    }
    reader.readAsDataURL(file)
  }

  const handleSyncMojang = async (): Promise<void> => {
    if (!account) return
    try {
      const updated = await syncProfile.mutateAsync({ accountId: account.id })
      if (updated.skinUrl) {
        const resolvedUrl = resolveSkinUrl(updated.skinUrl) ?? updated.skinUrl
        setSelection({
          id: 'microsoft',
          name: `${updated.username} (Mojang)`,
          skinUrl: resolvedUrl,
        })
        setSelectedSkinType('microsoft')
      }
      if (updated.skinVariant) {
        setArmModel(updated.skinVariant === 'slim' ? 'slim' : 'classic')
        setSkinSlim(updated.skinVariant === 'slim')
      }
      const capeCount = updated.capes?.length ?? 0
      pushToast(
        `Profil Minecraft "${updated.username}" berhasil disinkronkan! Ditemukan ${capeCount} jubah resmi.`,
      )
    } catch (err) {
      pushToast(`Gagal menyinkronkan profil Mojang: ${String(err)}`, 'error')
    }
  }

  const handleSelectCape = async (capeId: string | undefined): Promise<void> => {
    if (!account) {
      pushToast('Silakan pilih akun terlebih dahulu.', 'error')
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
        const name = getCapeDisplayName(found?.alias)
        pushToast(`Jubah "${name}" berhasil diaktifkan.`)
      } else {
        pushToast('Jubah dilepas.')
      }
    } catch (err) {
      pushToast(`Gagal mengubah jubah: ${String(err)}`, 'error')
    }
  }

  const handleCapeUpload = (event: ChangeEvent<HTMLInputElement>): void => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!file.name.toLowerCase().endsWith('.png')) {
      pushToast('Berkas jubah harus berupa PNG.', 'error')
      return
    }
    if (!account) {
      pushToast('Silakan pilih akun terlebih dahulu.', 'error')
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
        pushToast('Jubah kustom berhasil dipasang!')
      } catch (err) {
        pushToast(`Gagal memasang jubah kustom: ${String(err)}`, 'error')
      }
    }
    reader.readAsDataURL(file)
  }

  const handleInstallPack = async (): Promise<void> => {
    if (!account) {
      pushToast('Silakan pilih atau buat akun terlebih dahulu.', 'error')
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
        `Berhasil memasang "${res.packFileName}" ke ${res.updatedInstances} instance. Game akan memuat skin secara otomatis!`
      )
      setPackDialogOpen(false)
    } catch (err) {
      pushToast(`Gagal memasang resource pack: ${String(err)}`, 'error')
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

            <div className={styles.viewToggleRow} role="group" aria-label="Sudut pandang karakter">
              <button
                type="button"
                className={`${styles.viewToggleButton} ${!viewBack ? styles.viewToggleButtonActive : ''}`}
                onClick={() => setViewBack(false)}
              >
                Lihat Depan
              </button>
              <button
                type="button"
                className={`${styles.viewToggleButton} ${viewBack ? styles.viewToggleButtonActive : ''}`}
                onClick={() => setViewBack(true)}
              >
                Lihat Belakang
              </button>
            </div>

            <p className={styles.dragHint}>
              <Move size={13} aria-hidden />
              <span>Drag to rotate</span>
            </p>

            <div className={styles.armRow}>
              <span className={styles.armLabel}>Model lengan</span>
              <div className={styles.armToggle} role="radiogroup" aria-label="Model lengan">
                <button
                  type="button"
                  role="radio"
                  aria-checked={armModel === 'classic'}
                  className={`${styles.armOption} ${armModel === 'classic' ? styles.armOptionActive : ''}`}
                  onClick={() => applyArmModel('classic')}
                  data-testid="skin-model-classic"
                >
                  Klasik
                </button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={armModel === 'slim'}
                  className={`${styles.armOption} ${armModel === 'slim' ? styles.armOptionActive : ''}`}
                  onClick={() => applyArmModel('slim')}
                  data-testid="skin-model-slim"
                >
                  Slim
                </button>
              </div>
              <span className={styles.armHint}>
                {isSlim ? 'Lengan 3 px, seperti Alex' : 'Lengan 4 px, seperti Steve'}
              </span>
            </div>

            {activeSkin.capeUrl ? (
              <div className={styles.armRow}>
                <span className={styles.armLabel}>Jubah</span>
                <span className={styles.armHint} style={{ color: 'var(--color-primary, #10b981)', fontWeight: 500 }}>
                  Aktif ({account?.type === 'microsoft' ? 'Resmi Mojang' : 'Kustom'})
                </span>
              </div>
            ) : null}

            {account?.type === 'microsoft' ? (
              <button
                type="button"
                className={styles.syncMojangButton}
                onClick={handleSyncMojang}
                disabled={syncProfile.isPending}
                title="Ambil skin resmi dan daftar jubah yang Anda miliki langsung dari server Mojang"
              >
                <RefreshCw size={13} className={syncProfile.isPending ? styles.spin : ''} aria-hidden />
                <span>{syncProfile.isPending ? 'Menyinkronkan...' : 'Sinkronkan Skin & Jubah dari Mojang'}</span>
              </button>
            ) : null}

            <div className={styles.buttonRow}>
              <button
                type="button"
                className={styles.editSkinButton}
                onClick={() => setEditDialogOpen(true)}
              >
                <Pencil size={13} aria-hidden />
                <span>Edit skin</span>
              </button>
              <button
                type="button"
                className={styles.installPackButton}
                onClick={() => setPackDialogOpen(true)}
                title="Pasang Skin sebagai Resource Pack (Bisa untuk Vanilla & Modded)"
              >
                <Package size={13} aria-hidden />
                <span>Pasang Resource Pack</span>
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
                  Jubah (Capes) {account?.capes && account.capes.length > 0 ? `(${account.capes.length} Resmi)` : ''}
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
                    title="Tanpa Jubah"
                  >
                    {!activeSkin.capeUrl ? (
                      <span className={styles.checkBadge} aria-hidden>
                        <Check size={11} strokeWidth={3} />
                      </span>
                    ) : null}
                    <div className={styles.capeFigure}>
                      <ShieldOff size={24} style={{ opacity: 0.6 }} />
                    </div>
                    <span className={styles.capeName}>Tanpa Jubah</span>
                  </button>

                  {/* Official Mojang Capes */}
                  {account?.capes?.map((cape) => {
                    const isCapeActive = account.activeCapeId === cape.id || (cape.state === 'ACTIVE' && activeSkin.capeUrl)
                    const displayName = getCapeDisplayName(cape.alias)
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
                    title="Unggah berkas PNG jubah (64×32)"
                  >
                    <Plus size={20} className={styles.addIcon} aria-hidden />
                    <span className={styles.addTitle} style={{ fontSize: 11.5 }}>Jubah Kustom</span>
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
                <span className={styles.sectionTitle}>Saved skins</span>
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
                    <span className={styles.addTitle}>Add skin</span>
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
                <span className={styles.sectionTitle}>Default skins</span>
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
              {requiresSignIn
                ? 'Untuk akun offline di instance modded (Fabric/Forge/NeoForge/Quilt), launcher otomatis menyuntikkan CustomSkinLoader agar skin pilihan Anda tampil di dalam game.'
                : 'Saat bermain, game memakai skin resmi dari profil Microsoft Anda.'}
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
              <span className={styles.bannerTitle}>Akun Offline / Skin Kustom</span>
              <span className={styles.bannerDesc}>
                Skin Anda tersimpan di launcher dan otomatis aktif di instance modded via CustomSkinLoader. Masuk Microsoft jika ingin sinkronisasi resmi ke Mojang.
              </span>
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
            <span>Sign in to Microsoft</span>
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
        title="Editing skin"
        closeLabel="Close"
        size="lg"
        footer={
          <div className={styles.dialogActions}>
            <button
              type="button"
              className={styles.secondaryButton}
              onClick={() => setEditDialogOpen(false)}
            >
              <X size={13} aria-hidden />
              <span>Cancel</span>
            </button>
            <button
              type="button"
              className={styles.primaryButton}
              onClick={() => {
                setEditDialogOpen(false)
                setSkinSlim(armModel === 'slim')
                pushToast('Pilihan skin disimpan untuk launcher.')
              }}
            >
              <Check size={13} aria-hidden />
              <span>Selesai</span>
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
              <span>Drag to rotate</span>
            </p>
          </div>

          <div className={styles.capeColumn}>
            <span className={styles.capeLabel}>Model lengan</span>
            <div className={styles.armToggle} role="radiogroup" aria-label="Model lengan">
              <button
                type="button"
                role="radio"
                aria-checked={armModel === 'classic'}
                className={`${styles.armOption} ${armModel === 'classic' ? styles.armOptionActive : ''}`}
                onClick={() => applyArmModel('classic')}
                data-testid="skin-dialog-model-classic"
              >
                Klasik
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={armModel === 'slim'}
                className={`${styles.armOption} ${armModel === 'slim' ? styles.armOptionActive : ''}`}
                onClick={() => applyArmModel('slim')}
                data-testid="skin-dialog-model-slim"
              >
                Slim
              </button>
            </div>
            <p className={styles.capeHint}>
              Model 3 px dipakai skin seperti Alex, 4 px seperti Steve. Jubah dapat dipilih di daftar Jubah (Capes).
            </p>
          </div>
        </div>
      </GlassDialog>

      <GlassDialog
        open={packDialogOpen}
        onOpenChange={setPackDialogOpen}
        title="Pasang Skin Resource Pack"
        closeLabel="Tutup"
        size="md"
        footer={
          <div className={styles.dialogActions}>
            <button
              type="button"
              className={styles.secondaryButton}
              onClick={() => setPackDialogOpen(false)}
            >
              <X size={13} aria-hidden />
              <span>Batal</span>
            </button>
            <button
              type="button"
              className={styles.primaryButton}
              onClick={handleInstallPack}
              disabled={installResourcePack.isPending}
            >
              <Download size={13} aria-hidden />
              <span>{installResourcePack.isPending ? 'Memasang...' : 'Pasang ke Game'}</span>
            </button>
          </div>
        }
      >
        <div className={styles.dialogForm}>
          <p className={styles.dialogInfo}>
            Resource pack akan menimpa tekstur pemain default (Steve, Alex, dan karakter 1.20+) sehingga skin Anda otomatis tampil di Minecraft Vanilla maupun Modded tanpa memerlukan mod loader!
          </p>

          <label className={styles.dialogLabel} htmlFor="target-instance-select">
            Pilih Target Instance
          </label>
          <select
            id="target-instance-select"
            className={styles.dialogSelect}
            value={targetInstanceId}
            onChange={(e) => setTargetInstanceId(e.target.value)}
          >
            <option value="all">Semua Instance (All Instances)</option>
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
