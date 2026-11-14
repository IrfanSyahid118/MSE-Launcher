import { useRef, useState } from 'react'
import type { ChangeEvent, ReactNode } from 'react'
import { motion } from 'motion/react'
import { Check, Plus, Trash2, UserRound, Sparkles, Upload } from 'lucide-react'
import { useI18n } from '../i18n'
import { errorMessage } from '../api/launcher'
import {
  useAccounts,
  useActiveAccount,
  useSetActiveAccount,
  useSettings,
  useSignOut,
  useUpdateAccountSkin,
} from '../api/queries'
import { useAppStore } from '../stores/app-store'
import { AccountDialog } from '../components/AccountDialog/AccountDialog'
import { GlassButton } from '../components/GlassButton/GlassButton'
import { GlassCard } from '../components/GlassCard/GlassCard'
import { ScreenHeader } from '../components/ScreenHeader/ScreenHeader'
import { MinecraftSkin3D } from '../components/MinecraftSkin/MinecraftSkin3D'
import { useActiveSkin } from '../components/MinecraftSkin/useActiveSkin'
import styles from './Accounts.module.css'

export function Accounts(): ReactNode {
  const { t, language } = useI18n()
  const settingsQuery = useSettings()
  const accountsQuery = useAccounts()
  const { account: activeAccount } = useActiveAccount()
  const setActiveAccount = useSetActiveAccount()
  const signOut = useSignOut()
  const updateAccountSkin = useUpdateAccountSkin()
  const pushToast = useAppStore((state) => state.pushToast)
  const setSelectedSkin = useAppStore((state) => state.setSelectedSkin)
  const setSelectedSkinType = useAppStore((state) => state.setSelectedSkinType)
  const setCustomSkinUrl = useAppStore((state) => state.setCustomSkinUrl)
  const [createOpen, setCreateOpen] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const settings = settingsQuery.data
  const accounts = accountsQuery.data ?? []
  const activeSkin = useActiveSkin(activeAccount)

  const handleFileUpload = (e: ChangeEvent<HTMLInputElement>): void => {
    const file = e.target.files?.[0]
    e.target.value = ''
    if (!file) return
    const reader = new FileReader()
    reader.onload = (event) => {
      const dataUrl = event.target?.result
      if (typeof dataUrl === 'string') {
        setCustomSkinUrl(dataUrl)
        setSelectedSkinType('custom')
        if (activeAccount) {
          updateAccountSkin.mutate(
            {
              accountId: activeAccount.id,
              skinData: dataUrl,
              skinVariant: activeSkin.isSlim ? 'slim' : 'classic',
              capeData: activeAccount.capeUrl,
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
          pushToast(t('account.skin.loaded'))
        }
      }
    }
    reader.readAsDataURL(file)
  }

  const removeAccount = (accountId: string, username: string): void => {
    signOut.mutate(accountId, {
      onSuccess: () => {
        pushToast(t('account.signedOut', { username }))
      },
      onError: (error) => {
        pushToast(errorMessage(error, t('error.unknown')), 'error')
      },
    })
  }

  return (
    <div className={styles.page} data-testid="accounts-screen">
      <ScreenHeader
        title={t('account.manage')}
        subtitle={t('settings.accountsHint')}
        actions={
          <GlassButton
            variant="primary"
            icon={<Plus size={15} aria-hidden />}
            onClick={() => setCreateOpen(true)}
            data-testid="create-account-btn"
          >
            {t('account.create')}
          </GlassButton>
        }
      />

      <div className={styles.grid}>
        {/* Kolom Kiri: Daftar Akun */}
        <GlassCard tone="strong" className={styles.section}>
          <div className={styles.sectionHead}>
            <span className={styles.icon}>
              <UserRound size={17} aria-hidden />
            </span>
            <div>
              <h2 className={styles.sectionTitle}>{t('settings.accounts')}</h2>
              <p className={styles.sectionHint}>
                {t('account.registeredCount', { count: accounts.length })}
              </p>
            </div>
          </div>

          {accounts.length > 0 ? (
            <ul className={styles.accountList} data-testid="accounts-list">
              {accounts.map((account) => {
                const active = settings?.activeAccountId === account.id
                const created = new Date(account.createdAt)

                return (
                  <li key={account.id} className={`${styles.accountItem} ${active ? styles.accountItemActive : ''}`}>
                    <span className={styles.accountAvatar} aria-hidden>
                      {account.username.slice(0, 2).toUpperCase()}
                    </span>
                    <div className={styles.accountText}>
                      <span className={styles.accountName}>{account.username}</span>
                      <span className={styles.accountMeta}>
                        {account.type === 'offline'
                          ? t('account.type.offline')
                          : t('account.type.microsoft')}
                        {Number.isNaN(created.getTime())
                          ? ''
                          : ` · ${t('account.createdAt', {
                              date: new Intl.DateTimeFormat(language, { dateStyle: 'medium' }).format(created),
                            })}`}
                      </span>
                    </div>

                    <div className={styles.actions}>
                      {active ? (
                        <span className={styles.activeBadge}>
                          <Check size={13} aria-hidden />
                          {t('settings.accountsActive')}
                        </span>
                      ) : (
                        <GlassButton
                          size="sm"
                          variant="ghost"
                          onClick={() =>
                            setActiveAccount.mutate(account.id, {
                              onSuccess: () => {
                                pushToast(t('toast.accountActive', { username: account.username }))
                              },
                              onError: (error) => {
                                pushToast(errorMessage(error, t('error.unknown')), 'error')
                              },
                            })
                          }
                        >
                          {t('settings.accountsSelect')}
                        </GlassButton>
                      )}

                      <button
                        type="button"
                        className={styles.removeBtn}
                        onClick={() => removeAccount(account.id, account.username)}
                        aria-label={`${t('account.signOut')} ${account.username}`}
                        title={t('account.signOut')}
                      >
                        <Trash2 size={14} aria-hidden />
                      </button>
                    </div>
                  </li>
                )
              })}
            </ul>
          ) : (
            <div className={styles.empty}>
              <p className={styles.emptyTitle}>{t('account.none')}</p>
              <p className={styles.emptyHint}>{t('account.noneHint')}</p>
              <GlassButton
                variant="outline"
                size="sm"
                icon={<Plus size={14} aria-hidden />}
                onClick={() => setCreateOpen(true)}
              >
                {t('account.create')}
              </GlassButton>
            </div>
          )}

          <p className={styles.note}>{t('account.microsoftNote')}</p>
        </GlassCard>

        {/* Kolom Kanan: Kustomisasi Skin Minecraft */}
        <GlassCard tone="strong" className={styles.section}>
          <div className={styles.sectionHead}>
            <span className={styles.icon}>
              <Sparkles size={17} aria-hidden />
            </span>
            <div>
              <h2 className={styles.sectionTitle}>{t('account.skin.title')}</h2>
              <p className={styles.sectionHint}>{t('account.skin.hint')}</p>
            </div>
          </div>

          <div className={styles.skinCard}>
            <div className={styles.skinPreview}>
              <MinecraftSkin3D
                skinUrl={activeSkin.skinUrl}
                capeUrl={activeSkin.capeUrl}
                isSlim={activeSkin.isSlim}
                scale={0.68}
              />
            </div>

            <div className={styles.skinDetails}>
              <div className={styles.skinNameRow}>
                <span className={styles.skinName}>
                  {activeSkin.skinType === 'microsoft'
                    ? t('account.skin.microsoft')
                    : activeSkin.skinType === 'custom'
                      ? t('account.skin.custom')
                      : activeSkin.skinType === 'alex'
                        ? t('account.skin.alex')
                        : t('account.skin.steve')}
                </span>
                <span className={styles.skinBadge3d}>{t('account.skin.badge3d')}</span>
              </div>

              <p className={styles.skinDescription}>
                {activeSkin.skinType === 'microsoft'
                  ? t('account.skin.microsoftDesc')
                  : activeSkin.skinType === 'custom'
                    ? t('account.skin.customDesc')
                    : activeSkin.skinType === 'alex'
                      ? t('account.skin.alexDesc')
                      : t('account.skin.steveDesc')}
              </p>

              <p className={styles.dragHint}>{t('account.skin.dragHint')}</p>

              <div
                className={styles.segmented}
                role="radiogroup"
                aria-label={t('account.skin.modelLabel')}
              >
                {activeSkin.isMicrosoft ? (
                  <button
                    type="button"
                    className={`${styles.segment} ${activeSkin.skinType === 'microsoft' ? styles.segmentActive : ''}`}
                    aria-checked={activeSkin.skinType === 'microsoft'}
                    role="radio"
                    onClick={() => setSelectedSkinType('microsoft')}
                  >
                    {activeSkin.skinType === 'microsoft' ? (
                      <motion.div
                        layoutId="accountsSkinPill"
                        className={styles.activeSegmentPill}
                        transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                      />
                    ) : null}
                    <span className={styles.segmentLabel}>{t('account.skin.model.microsoft')}</span>
                  </button>
                ) : null}

                <button
                  type="button"
                  className={`${styles.segment} ${activeSkin.skinType === 'steve' ? styles.segmentActive : ''}`}
                  aria-checked={activeSkin.skinType === 'steve'}
                  role="radio"
                  onClick={() => {
                    setSelectedSkin('steve')
                    setSelectedSkinType('steve')
                  }}
                >
                  {activeSkin.skinType === 'steve' ? (
                    <motion.div
                      layoutId="accountsSkinPill"
                      className={styles.activeSegmentPill}
                      transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                    />
                  ) : null}
                  <span className={styles.segmentLabel}>{t('account.skin.model.steve')}</span>
                </button>

                <button
                  type="button"
                  className={`${styles.segment} ${activeSkin.skinType === 'alex' ? styles.segmentActive : ''}`}
                  aria-checked={activeSkin.skinType === 'alex'}
                  role="radio"
                  onClick={() => {
                    setSelectedSkin('alex')
                    setSelectedSkinType('alex')
                  }}
                >
                  {activeSkin.skinType === 'alex' ? (
                    <motion.div
                      layoutId="accountsSkinPill"
                      className={styles.activeSegmentPill}
                      transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                    />
                  ) : null}
                  <span className={styles.segmentLabel}>{t('account.skin.model.alex')}</span>
                </button>

                <button
                  type="button"
                  className={`${styles.segment} ${activeSkin.skinType === 'custom' ? styles.segmentActive : ''}`}
                  aria-checked={activeSkin.skinType === 'custom'}
                  role="radio"
                  onClick={() => {
                    if (!activeSkin.hasCustom) {
                      fileInputRef.current?.click()
                    } else {
                      setSelectedSkinType('custom')
                    }
                  }}
                >
                  {activeSkin.skinType === 'custom' ? (
                    <motion.div
                      layoutId="accountsSkinPill"
                      className={styles.activeSegmentPill}
                      transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                    />
                  ) : null}
                  <span className={styles.segmentLabel}>{t('account.skin.model.custom')}</span>
                </button>
              </div>

              <div className={styles.uploadRow}>
                <GlassButton
                  size="sm"
                  variant="outline"
                  icon={<Upload size={13} aria-hidden />}
                  onClick={() => fileInputRef.current?.click()}
                >
                  {t('account.skin.upload')}
                </GlassButton>
                <input
                  ref={fileInputRef}
                  type="file"
                  accept="image/png"
                  style={{ display: 'none' }}
                  onChange={handleFileUpload}
                />
              </div>
            </div>
          </div>
        </GlassCard>
      </div>

      <AccountDialog open={createOpen} onOpenChange={setCreateOpen} />
    </div>
  )
}
