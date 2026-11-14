import { useRef, useState } from 'react'
import type { ChangeEvent, ReactNode } from 'react'
import { useNavigate } from 'react-router'
import { ChevronDown, Plus, Upload, UserRound } from 'lucide-react'
import { motion } from 'motion/react'
import type { Account, Instance, MemoryInfo, MinecraftVersionInfo, Settings } from '@mc/shared'
import { useI18n } from '../../i18n'
import { useAppStore } from '../../stores/app-store'
import { AccountDialog } from '../AccountDialog/AccountDialog'
import { AccountMenu } from '../AccountMenu/AccountMenu'
import { GlassButton } from '../GlassButton/GlassButton'
import { MinecraftSkin3D } from '../MinecraftSkin/MinecraftSkin3D'
import { useActiveSkin } from '../MinecraftSkin/useActiveSkin'
import { ProgressBar } from '../ProgressBar/ProgressBar'
import styles from './AccountCard.module.css'

export interface AccountCardProps {
  account: Account | null
  instance: Instance | null
  version: MinecraftVersionInfo | null
  settings: Settings
  memory: MemoryInfo | undefined
}

/**
 * Panel kanan: akun aktif, skin 3D Minecraft (Steve, Alex, Custom, Microsoft),
 * tombol menu khusus akun, dan ringkasan launcher (versi terpilih, alokasi RAM, status).
 */
export function AccountCard({ account, instance, version, settings, memory }: AccountCardProps): ReactNode {
  const { t } = useI18n()
  const navigate = useNavigate()
  const [createOpen, setCreateOpen] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const activeSkin = useActiveSkin(account)
  const setSelectedSkin = useAppStore((state) => state.setSelectedSkin)
  const setSelectedSkinType = useAppStore((state) => state.setSelectedSkinType)
  const setCustomSkinUrl = useAppStore((state) => state.setCustomSkinUrl)

  const maxMb = settings.memory.defaultMaxMb
  const totalMb = memory?.totalMb ?? null
  const share = totalMb && totalMb > 0 ? Math.min(100, (maxMb / totalMb) * 100) : null
  const ready = Boolean(account) && Boolean(instance) && Boolean(version?.installed)

  const isExpired = account?.type === 'microsoft' && account.expiresAt && new Date(account.expiresAt) < new Date()

  const handleFileUpload = (e: ChangeEvent<HTMLInputElement>): void => {
    const file = e.target.files?.[0]
    if (!file) return
    const reader = new FileReader()
    reader.onload = (event) => {
      const dataUrl = event.target?.result
      if (typeof dataUrl === 'string') {
        setCustomSkinUrl(dataUrl)
        setSelectedSkinType('custom')
      }
    }
    reader.readAsDataURL(file)
  }

  const accountPill = (
    <span className={styles.typePill}>
      {account
        ? account.type === 'offline'
          ? t('account.type.offline')
          : t('account.type.microsoft')
        : t('account.none')}
      <ChevronDown size={13} aria-hidden />
    </span>
  )

  return (
    <aside className={styles.card} aria-label={t('account.summary')}>
      <div className={styles.profile}>
        <div className={styles.skinFrame} title={t('account.skin.dragHint')}>
          <MinecraftSkin3D
            skinUrl={activeSkin.skinUrl}
            capeUrl={activeSkin.capeUrl}
            isSlim={activeSkin.isSlim}
            scale={0.5}
          />
          <span className={styles.skin3dBadge}>3D</span>
        </div>

        <div className={styles.identity}>
          <span className={styles.username} data-testid="account-username">
            {account?.username ?? t('account.none')}
          </span>
          {account ? (
            <AccountMenu
              trigger={
                <button type="button" className={styles.pillButton} aria-label={t('account.manage')}>
                  {accountPill}
                </button>
              }
            />
          ) : (
            accountPill
          )}

          {/* Pengganti Skin: Microsoft / Steve / Alex / Custom */}
          <div className={styles.skinSelector} role="radiogroup" aria-label={t('account.skin.modelLabel')}>
            {activeSkin.isMicrosoft ? (
              <button
                type="button"
                className={`${styles.skinPillBtn} ${activeSkin.skinType === 'microsoft' ? styles.skinPillBtnActive : ''}`}
                role="radio"
                aria-checked={activeSkin.skinType === 'microsoft'}
                onClick={() => setSelectedSkinType('microsoft')}
                title={t('account.skin.microsoft')}
              >
                {activeSkin.skinType === 'microsoft' ? (
                  <motion.div
                    layoutId="homeSkinPill"
                    className={styles.activePill}
                    transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                  />
                ) : null}
                <span className={styles.skinPillLabel}>MS</span>
              </button>
            ) : null}

            <button
              type="button"
              className={`${styles.skinPillBtn} ${activeSkin.skinType === 'steve' ? styles.skinPillBtnActive : ''}`}
              role="radio"
              aria-checked={activeSkin.skinType === 'steve'}
              onClick={() => {
                setSelectedSkin('steve')
                setSelectedSkinType('steve')
              }}
              title={t('account.skin.steve')}
            >
              {activeSkin.skinType === 'steve' ? (
                <motion.div
                  layoutId="homeSkinPill"
                  className={styles.activePill}
                  transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                />
              ) : null}
              <span className={styles.skinPillLabel}>Steve</span>
            </button>

            <button
              type="button"
              className={`${styles.skinPillBtn} ${activeSkin.skinType === 'alex' ? styles.skinPillBtnActive : ''}`}
              role="radio"
              aria-checked={activeSkin.skinType === 'alex'}
              onClick={() => {
                setSelectedSkin('alex')
                setSelectedSkinType('alex')
              }}
              title={t('account.skin.alex')}
            >
              {activeSkin.skinType === 'alex' ? (
                <motion.div
                  layoutId="homeSkinPill"
                  className={styles.activePill}
                  transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                />
              ) : null}
              <span className={styles.skinPillLabel}>Alex</span>
            </button>

            <button
              type="button"
              className={`${styles.skinPillBtn} ${activeSkin.skinType === 'custom' ? styles.skinPillBtnActive : ''}`}
              role="radio"
              aria-checked={activeSkin.skinType === 'custom'}
              onClick={() => {
                if (!activeSkin.hasCustom) {
                  fileInputRef.current?.click()
                } else {
                  setSelectedSkinType('custom')
                }
              }}
              title={t('account.skin.custom')}
            >
              {activeSkin.skinType === 'custom' ? (
                <motion.div
                  layoutId="homeSkinPill"
                  className={styles.activePill}
                  transition={{ type: 'spring', stiffness: 450, damping: 35 }}
                />
              ) : null}
              <span className={styles.skinPillLabel}>Custom</span>
            </button>
          </div>

          <button
            type="button"
            className={styles.uploadLink}
            onClick={() => fileInputRef.current?.click()}
            title={t('account.skin.upload')}
          >
            <Upload size={11} aria-hidden />
            <span>{t('account.skin.uploadShort')}</span>
          </button>
          <input
            ref={fileInputRef}
            type="file"
            accept="image/png"
            style={{ display: 'none' }}
            onChange={handleFileUpload}
          />
        </div>
      </div>

      {/* Tombol Menu Khusus Panel Akun */}
      <div className={styles.accountActions}>
        <GlassButton
          size="sm"
          variant="outline"
          icon={<UserRound size={13} aria-hidden />}
          onClick={() => navigate('/accounts')}
          className={styles.manageBtn}
        >
          {t('account.manage')}
        </GlassButton>
        <GlassButton
          size="sm"
          variant="ghost"
          icon={<Plus size={13} aria-hidden />}
          onClick={() => setCreateOpen(true)}
        >
          {t('account.createTitle')}
        </GlassButton>
      </div>

      {account ? null : (
        <div className={styles.empty}>
          <p className={styles.emptyHint}>{t('account.noneHint')}</p>
          <GlassButton variant="outline" size="sm" icon={<Plus size={14} aria-hidden />} onClick={() => setCreateOpen(true)}>
            {t('account.create')}
          </GlassButton>
        </div>
      )}

      <div className={styles.summary}>
        <h2 className={styles.summaryTitle}>{t('account.summary')}</h2>

        <div className={styles.row}>
          <span className={styles.rowLabel}>{t('account.selectedInstance')}</span>
          <span className={styles.rowValue} data-testid="summary-instance">
            {instance?.name ?? t('common.none')}
          </span>
        </div>

        <div className={styles.row}>
          <span className={styles.rowLabel}>{t('account.selectedVersion')}</span>
          <span className={styles.rowValue} data-testid="summary-version">
            {version?.id ?? t('common.none')}
          </span>
        </div>

        <div className={styles.row}>
          <span className={styles.rowLabel}>{t('account.memory')}</span>
          <span className={styles.rowValue}>
            {totalMb
              ? t('account.memoryOf', { used: maxMb, total: totalMb })
              : t('account.memoryUnknown', { used: maxMb })}
          </span>
        </div>
        {share === null ? null : (
          <ProgressBar percent={share} label={t('account.memory')} />
        )}

        <div className={styles.row}>
          <span className={styles.rowLabel}>{t('account.status')}</span>
          <span className={styles.status}>
            <span className={ready ? styles.dotReady : styles.dotWait} aria-hidden />
            {ready ? t('account.status.ready') : t('account.status.noVersion')}
          </span>
        </div>

        {isExpired ? (
          <p className={styles.expiredWarning} role="alert">
            {t('account.microsoftExpired')}
          </p>
        ) : null}
      </div>

      <AccountDialog open={createOpen} onOpenChange={setCreateOpen} />
    </aside>
  )
}
