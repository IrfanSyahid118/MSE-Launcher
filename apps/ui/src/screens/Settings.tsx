import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import {
  AlertCircle,
  Check,
  Cpu,
  Download,
  Languages,
  Palette,
  Radio,
  RefreshCw,
} from 'lucide-react'
import type { JvmPreset, Language, SettingsPatch, ThemeMode, UpdateCheckResult } from '@mc/shared'
import { useI18n } from '../i18n'
import { errorMessage, invoke } from '../api/launcher'
import {
  useJavaInstallations,
  useMemoryInfo,
  useSettings,
  useUpdateSettings,
  useValidateJava,
} from '../api/queries'
import { consumeBufferedTaskEvent, useAppStore } from '../stores/app-store'
import { GlassButton } from '../components/GlassButton/GlassButton'
import { GlassSlider } from '../components/GlassSlider/GlassSlider'
import { GlassSwitch } from '../components/GlassSwitch/GlassSwitch'
import { Spinner } from '../components/Spinner/Spinner'
import styles from './Settings.module.css'

const MEMORY_STEP = 256
const MEMORY_FLOOR = 512
const PARALLEL_MIN = 1
const PARALLEL_MAX = 16

type Category = 'appearance' | 'language' | 'java' | 'downloads' | 'discord'

const APP_VERSION = '1.0.0'

function platformLabel(): string {
  const agent = navigator.userAgent
  if (/Windows NT 10/.test(agent)) return 'Windows 10 / 11'
  if (/Windows NT/.test(agent)) return 'Windows'
  if (/Mac OS X/.test(agent)) return 'macOS'
  if (/Linux/.test(agent)) return 'Linux'
  return 'Desktop'
}

export function Settings(): ReactNode {
  const { t } = useI18n()
  const settingsQuery = useSettings()
  const memoryQuery = useMemoryInfo()
  const javaQuery = useJavaInstallations()
  const validateJava = useValidateJava()
  const updateSettings = useUpdateSettings()
  const pushToast = useAppStore((state) => state.pushToast)
  const [activeCategory, setActiveCategory] = useState<Category>('appearance')
  const settings = settingsQuery.data
  const totalMb = memoryQuery.data?.totalMb ?? null
  const freeMb = memoryQuery.data?.freeMb ?? null

  const [maxMb, setMaxMb] = useState(settings?.memory.defaultMaxMb ?? MEMORY_FLOOR)
  const [minMb, setMinMb] = useState(settings?.memory.defaultMinMb ?? MEMORY_FLOOR)
  const [parallelism, setParallelism] = useState(settings?.downloads.parallelism ?? 1)

  const [customPathInput, setCustomPathInput] = useState(settings?.java.customPath ?? '')
  const [customJvmArgsInput, setCustomJvmArgsInput] = useState(settings?.java.customJvmArgs ?? '')
  const customPathDirtyRef = useRef(false)
  const customJvmArgsDirtyRef = useRef(false)
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null)
  const [checkingUpdate, setCheckingUpdate] = useState(false)
  const [updateCheckedOnce, setUpdateCheckedOnce] = useState(false)
  const [updateTaskId, setUpdateTaskId] = useState<string | null>(null)
  const [updateErrorMsg, setUpdateErrorMsg] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    void invoke('updater:check', undefined)
      .then((result) => {
        if (!active) return
        setUpdateInfo(result)
        setUpdateCheckedOnce(true)
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [])

  useEffect(() => {
    if (!updateTaskId) return
    const offDone = window.launcher.on('task:done', ({ taskId }) => {
      if (taskId !== updateTaskId) return
      setUpdateTaskId(null)
    })
    const offError = window.launcher.on('task:error', ({ taskId, error }) => {
      if (taskId !== updateTaskId) return
      setUpdateTaskId(null)
      setUpdateErrorMsg(error.message)
      pushToast(error.message, 'error')
    })
    return () => {
      offDone()
      offError()
    }
  }, [pushToast, updateTaskId])

  useEffect(() => {
    if (!settings) return
    const limit = totalMb ?? settings.memory.defaultMaxMb
    setMaxMb(Math.min(settings.memory.defaultMaxMb, limit))
    setMinMb(Math.min(settings.memory.defaultMinMb, limit))
    setParallelism(settings.downloads.parallelism)
    if (!customPathDirtyRef.current) {
      setCustomPathInput(settings.java.customPath ?? '')
    }
    if (!customJvmArgsDirtyRef.current) {
      setCustomJvmArgsInput(settings.java.customJvmArgs ?? '')
    }
  }, [settings, totalMb])

  if (!settings) {
    return null
  }

  const save = (patch: SettingsPatch): void => {
    updateSettings.mutate(patch, {
      onError: (error) => {
        pushToast(errorMessage(error, t('toast.settingsError')), 'error')
      },
    })
  }

  const saveMemory = (max: number, min: number): void => {
    save({ memory: { defaultMaxMb: max, defaultMinMb: Math.min(min, max) } })
  }

  const isUpdating = updateTaskId !== null
  const modeLabel =
    updateInfo?.installMode === 'installer'
      ? t('settings.modeInstaller')
      : t('settings.modePortable')

  const handleCheckUpdates = async (): Promise<void> => {
    if (checkingUpdate || isUpdating) return
    setCheckingUpdate(true)
    setUpdateErrorMsg(null)
    try {
      const result = await invoke('updater:check', undefined)
      setUpdateInfo(result)
      setUpdateCheckedOnce(true)
    } catch (error) {
      const msg = errorMessage(error, t('settings.updateError'))
      setUpdateErrorMsg(msg)
      pushToast(msg, 'error')
    } finally {
      setCheckingUpdate(false)
    }
  }

  const handleInstallUpdate = async (): Promise<void> => {
    if (!updateInfo?.downloadUrl || !updateInfo.assetName || !updateInfo.sha256 || isUpdating) return
    setUpdateErrorMsg(null)
    try {
      const res = await invoke('updater:install', {
        downloadUrl: updateInfo.downloadUrl,
        assetName: updateInfo.assetName,
        sha256: updateInfo.sha256,
        sizeBytes: updateInfo.sizeBytes,
      })
      const buffered = consumeBufferedTaskEvent(res.taskId)
      if (buffered?.error) {
        setUpdateErrorMsg(buffered.error.message)
        pushToast(buffered.error.message, 'error')
        return
      }
      if (buffered?.done) {
        return
      }
      setUpdateTaskId(res.taskId)
    } catch (error) {
      const msg = errorMessage(error, t('settings.updateError'))
      setUpdateErrorMsg(msg)
      pushToast(msg, 'error')
    }
  }

  const themeModes: { mode: ThemeMode; label: string }[] = [
    { mode: 'dark', label: t('settings.dark') },
    { mode: 'light', label: t('settings.light') },
    { mode: 'system', label: t('settings.syncSystem') },
  ]

  const languages: { code: Language; label: string }[] = [
    { code: 'id', label: t('settings.language.id') },
    { code: 'en', label: t('settings.language.en') },
  ]

  const activeJava = javaQuery.data?.find((runtime) =>
    settings.java.customPath
      ? runtime.path === settings.java.customPath
      : settings.java.preferManaged
        ? runtime.managed
        : true,
  )
  const is32Bit = activeJava?.bits === 32
  const effectiveMaxAllowed = is32Bit ? Math.min(totalMb ?? 1024, 1024) : (totalMb ?? 262_144)
  const memoryHigh = totalMb !== null && maxMb > totalMb * 0.75

  const jvmPresets: { preset: JvmPreset; label: string }[] = [
    { preset: 'default', label: t('settings.jvmPresetDefault') },
    { preset: 'performance', label: t('settings.jvmPresetPerformance') },
    { preset: 'low-memory', label: t('settings.jvmPresetLowMemory') },
    { preset: 'custom', label: t('settings.jvmPresetCustom') },
  ]

  const navGroups: { label: string; items: { id: Category; label: string; icon: ReactNode }[] }[] = [
    {
      label: t('settings.groupDisplay'),
      items: [
        { id: 'appearance', label: t('settings.appearance'), icon: <Palette size={15} aria-hidden /> },
        { id: 'language', label: t('settings.language'), icon: <Languages size={15} aria-hidden /> },
      ],
    },
    {
      label: t('settings.groupInstanceDefaults'),
      items: [
        {
          id: 'java',
          label: t('settings.javaAndMemory'),
          icon: <Cpu size={15} aria-hidden />,
        },
        {
          id: 'downloads',
          label: t('settings.downloads'),
          icon: <Download size={15} aria-hidden />,
        },
      ],
    },
    {
      label: t('settings.groupIntegrations'),
      items: [
        {
          id: 'discord',
          label: t('settings.discord'),
          icon: <Radio size={15} aria-hidden />,
        },
      ],
    },
  ]

  return (
    <div className={styles.page} data-testid="settings-screen">
      <div className={styles.layout}>
        <nav className={styles.nav} aria-label={t('settings.navAria')}>
          {navGroups.map((group) => (
            <div key={group.label} className={styles.navGroup}>
              <span className={styles.navGroupLabel}>{group.label}</span>
              {group.items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={`${styles.navItem} ${activeCategory === item.id ? styles.navItemActive : ''}`}
                  onClick={() => setActiveCategory(item.id)}
                  aria-current={activeCategory === item.id ? 'page' : undefined}
                  data-testid={`settings-nav-${item.id}`}
                >
                  <span className={styles.navIcon}>{item.icon}</span>
                  <span>{item.label}</span>
                </button>
              ))}
            </div>
          ))}

          <div className={styles.navFooter}>
            <div className={styles.navFooterHead}>
              <span className={styles.navFooterName}>{t('app.name')} {APP_VERSION}</span>
              {updateInfo ? (
                <span className={styles.badgeMuted} data-testid="install-mode-badge">
                  {modeLabel}
                </span>
              ) : null}
            </div>
            <span className={styles.navFooterMeta}>{platformLabel()}</span>

            <div className={styles.navFooterUpdate}>
              {isUpdating ? (
                <span className={styles.navFooterStatus}>{t('settings.updating')}</span>
              ) : updateErrorMsg ? (
                <span className={`${styles.navFooterStatus} ${styles.navFooterStatusError}`}>
                  {updateErrorMsg}
                </span>
              ) : updateInfo?.updateAvailable ? (
                <span className={`${styles.navFooterStatus} ${styles.navFooterStatusAvailable}`}>
                  {t('settings.updateAvailable', {
                    version: updateInfo.latestVersion,
                    mode: modeLabel,
                  })}
                </span>
              ) : updateCheckedOnce ? (
                <span className={styles.navFooterStatus}>{t('settings.upToDate')}</span>
              ) : null}

              {updateInfo?.updateAvailable &&
              updateInfo.downloadUrl &&
              updateInfo.assetName &&
              updateInfo.sha256 ? (
                <GlassButton
                  size="sm"
                  variant="primary"
                  disabled={isUpdating}
                  icon={isUpdating ? <Spinner size={12} /> : <Download size={12} aria-hidden />}
                  data-testid="update-install-btn"
                  onClick={() => void handleInstallUpdate()}
                >
                  {isUpdating ? t('settings.updating') : t('settings.updateNow')}
                </GlassButton>
              ) : updateInfo?.updateAvailable ? (
                <GlassButton
                  size="sm"
                  variant="ghost"
                  data-testid="update-manual-btn"
                  title={t('settings.updateNoChecksum')}
                  onClick={() => {
                    void invoke('shell:openExternal', {
                      url: 'https://github.com/IrfanSyahid118/MSE-Launcher/releases',
                    }).catch(() => undefined)
                  }}
                >
                  {t('settings.updateManual')}
                </GlassButton>
              ) : (
                <GlassButton
                  size="sm"
                  variant="ghost"
                  disabled={checkingUpdate || isUpdating}
                  icon={checkingUpdate ? <Spinner size={12} /> : <RefreshCw size={12} aria-hidden />}
                  data-testid="update-check-btn"
                  onClick={() => void handleCheckUpdates()}
                >
                  {checkingUpdate ? t('settings.checkingUpdate') : t('settings.checkUpdate')}
                </GlassButton>
              )}
            </div>

            <p className={styles.navFooterLegal}>{t('settings.mojangDisclaimer')}</p>
          </div>
        </nav>

        <div className={styles.content}>
          <header className={styles.contentHeader}>
            <h1 className={styles.contentTitle}>{t('settings.title')}</h1>
            <p className={styles.contentHint}>{t('settings.subtitle')}</p>
          </header>

          {activeCategory === 'appearance' ? (
            <section className={styles.panel}>
              <div className={styles.panelHead}>
                <h2 className={styles.panelTitle}>{t('settings.colorTheme')}</h2>
                <p className={styles.panelHint}>{t('settings.colorThemeHint')}</p>
              </div>

              <div className={styles.themeGrid}>
                {themeModes.map(({ mode, label }) => {
                  const isSelected = settings.theme.mode === mode
                  return (
                    <button
                      key={mode}
                      type="button"
                      className={`${styles.themeCard} ${isSelected ? styles.themeCardActive : ''}`}
                      aria-pressed={isSelected}
                      data-testid={`theme-${mode}`}
                      onClick={() => save({ theme: { mode } })}
                    >
                      <span className={`${styles.themePreview} ${styles[`preview_${mode}`]}`} aria-hidden>
                        <span className={styles.previewSide} />
                        <span className={styles.previewBody}>
                          <span className={styles.previewLine} />
                          <span className={`${styles.previewLine} ${styles.previewLineShort}`} />
                        </span>
                      </span>
                      <span className={styles.themeLabel}>
                        <span className={`${styles.radio} ${isSelected ? styles.radioOn : ''}`} aria-hidden />
                        <span>{label}</span>
                      </span>
                    </button>
                  )
                })}
              </div>

              <div className={styles.divider} />

              <div className={styles.switchRow}>
                <div className={styles.switchText}>
                  <span className={styles.fieldLabel}>{t('settings.reduceTransparency')}</span>
                  <p className={styles.rowHint}>{t('settings.reduceTransparencyHint')}</p>
                </div>
                <GlassSwitch
                  checked={settings.theme.reduceTransparency}
                  onCheckedChange={(checked) => save({ theme: { reduceTransparency: checked } })}
                  label={t('settings.reduceTransparency')}
                />
              </div>

              <div className={styles.switchRow}>
                <div className={styles.switchText}>
                  <span className={styles.fieldLabel}>{t('settings.reduceMotion')}</span>
                  <p className={styles.rowHint}>{t('settings.reduceMotionHint')}</p>
                </div>
                <GlassSwitch
                  checked={settings.theme.reduceMotion}
                  onCheckedChange={(checked) => save({ theme: { reduceMotion: checked } })}
                  label={t('settings.reduceMotion')}
                />
              </div>
            </section>
          ) : null}

          {activeCategory === 'language' ? (
            <section className={styles.panel}>
              <div className={styles.panelHead}>
                <h2 className={styles.panelTitle}>{t('settings.language')}</h2>
                <p className={styles.panelHint}>{t('settings.languageHint')}</p>
              </div>

              <div className={styles.languageGrid}>
                {languages.map(({ code, label }) => {
                  const isSelected = settings.language === code
                  return (
                    <button
                      key={code}
                      type="button"
                      className={`${styles.languageCard} ${isSelected ? styles.languageCardActive : ''}`}
                      aria-pressed={isSelected}
                      data-testid={`language-${code}`}
                      onClick={() => save({ language: code })}
                    >
                      <span>{label}</span>
                      {isSelected ? (
                        <span className={styles.languageCheck}>
                          <Check size={12} aria-hidden />
                        </span>
                      ) : null}
                    </button>
                  )
                })}
              </div>
            </section>
          ) : null}

          {activeCategory === 'java' ? (
            <section className={styles.panel} data-testid="java-settings-card">
              <div className={styles.panelHead}>
                <h2 className={styles.panelTitle}>{t('settings.javaAndMemory')}</h2>
                <p className={styles.panelHint}>{t('settings.memoryHint')}</p>
              </div>

              {totalMb === null ? (
                <p className={styles.warning}>{t('settings.memoryUnknown')}</p>
              ) : (
                <>
                  <div className={styles.valueRow}>
                    <span className={styles.fieldLabel}>{t('settings.memoryMax')}</span>
                    <span className={styles.value}>{maxMb} MB</span>
                  </div>
                  <GlassSlider
                    value={Math.min(maxMb, effectiveMaxAllowed)}
                    min={MEMORY_FLOOR}
                    max={Math.max(MEMORY_FLOOR, effectiveMaxAllowed)}
                    step={MEMORY_STEP}
                    label={t('settings.memoryMax')}
                    onValueChange={(value) => setMaxMb(Math.min(value, effectiveMaxAllowed))}
                    onValueCommit={(value) => saveMemory(Math.min(value, effectiveMaxAllowed), minMb)}
                  />

                  <div className={styles.valueRow}>
                    <span className={styles.fieldLabel}>{t('settings.memoryMin')}</span>
                    <span className={styles.value}>{Math.min(minMb, maxMb)} MB</span>
                  </div>
                  <GlassSlider
                    value={Math.min(minMb, maxMb)}
                    min={MEMORY_FLOOR}
                    max={Math.max(MEMORY_FLOOR, maxMb)}
                    step={MEMORY_STEP}
                    label={t('settings.memoryMin')}
                    onValueChange={setMinMb}
                    onValueCommit={(value) => saveMemory(maxMb, value)}
                  />

                  <p className={styles.rowHint}>
                    {t('settings.memoryTotal', { total: totalMb })}
                    {freeMb === null ? '' : ` \u00b7 ${t('settings.memoryFree', { free: freeMb })}`}
                  </p>
                  {memoryHigh ? <p className={styles.warning}>{t('settings.memoryHigh')}</p> : null}
                  {is32Bit ? (
                    <p className={styles.warning} data-testid="memory-32bit-warning">
                      {t('settings.java32BitWarning')}
                    </p>
                  ) : null}
                </>
              )}

              <div className={styles.divider} />

              <div className={styles.switchRow}>
                <div className={styles.switchText}>
                  <span className={styles.fieldLabel}>{t('settings.javaPreferManaged')}</span>
                  <p className={styles.rowHint}>{t('settings.javaPreferManagedHint')}</p>
                </div>
                <GlassSwitch
                  checked={settings.java.preferManaged}
                  onCheckedChange={(checked) => save({ java: { preferManaged: checked } })}
                  label={t('settings.javaPreferManaged')}
                />
              </div>

              <div className={styles.field}>
                <div className={styles.fieldHead}>
                  <span className={styles.fieldLabel}>{t('settings.javaDetected')}</span>
                  <GlassButton
                    size="sm"
                    variant="ghost"
                    icon={javaQuery.isFetching ? <Spinner size={12} /> : <RefreshCw size={12} aria-hidden />}
                    disabled={javaQuery.isFetching}
                    onClick={() => void javaQuery.refetch()}
                  >
                    {t('settings.javaRedetect')}
                  </GlassButton>
                </div>

                {javaQuery.isLoading ? (
                  <p className={styles.rowHint}>{t('settings.javaDetecting')}</p>
                ) : javaQuery.data && javaQuery.data.length > 0 ? (
                  <ul className={styles.runtimeList} data-testid="java-runtimes-list">
                    {javaQuery.data.map((runtime) => {
                      const isSelected =
                        settings.java.customPath === runtime.path ||
                        (settings.java.preferManaged && runtime.managed)
                      return (
                        <li key={runtime.path} className={styles.runtimeItem}>
                          <span className={styles.runtimeText}>
                            <span className={styles.runtimeName}>
                              Java {runtime.majorVersion} ({runtime.version})
                              {runtime.managed ? (
                                <span className={styles.badgeMuted}>{t('settings.mojangBadge')}</span>
                              ) : null}
                              <span className={styles.badgeMuted}>
                                {runtime.bits === 32 ? '32-bit' : '64-bit'}
                              </span>
                            </span>
                            <span className={styles.runtimeMeta}>
                              {runtime.vendor ?? 'Java'} {'\u00b7'} {runtime.path}
                            </span>
                          </span>
                          {isSelected ? (
                            <span className={styles.activeBadge}>
                              <Check size={11} aria-hidden />
                              {t('settings.accountsActive')}
                            </span>
                          ) : (
                            <GlassButton
                              size="sm"
                              variant="ghost"
                              onClick={() =>
                                save({ java: { preferManaged: false, customPath: runtime.path } })
                              }
                            >
                              {t('settings.accountsSelect')}
                            </GlassButton>
                          )}
                        </li>
                      )
                    })}
                  </ul>
                ) : (
                  <p className={styles.empty}>{t('settings.javaNoneDetected')}</p>
                )}
              </div>

              <div className={styles.field}>
                <span className={styles.fieldLabel}>{t('settings.javaCustomPath')}</span>
                <p className={styles.rowHint}>{t('settings.javaCustomPathHint')}</p>
                <div className={styles.inputRow}>
                  <input
                    type="text"
                    className={styles.textInput}
                    value={customPathInput}
                    placeholder={t('settings.javaCustomPlaceholder')}
                    onChange={(event) => {
                      customPathDirtyRef.current = true
                      setCustomPathInput(event.target.value)
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' && customPathInput.trim()) {
                        validateJava.mutate(customPathInput.trim(), {
                          onSuccess: (result) => {
                            customPathDirtyRef.current = false
                            save({
                              java: { customPath: customPathInput.trim(), preferManaged: false },
                            })
                            pushToast(
                              t('settings.javaValid', {
                                version: result.version,
                                bits: result.bits,
                                vendor: result.vendor ?? 'Java',
                              }),
                            )
                          },
                          onError: (error) => {
                            pushToast(errorMessage(error, t('error.unknown')), 'error')
                          },
                        })
                      }
                    }}
                  />
                  <GlassButton
                    size="sm"
                    variant="ghost"
                    disabled={!customPathInput.trim() || validateJava.isPending}
                    onClick={() => {
                      validateJava.mutate(customPathInput.trim(), {
                        onSuccess: (result) => {
                          customPathDirtyRef.current = false
                          save({
                            java: { customPath: customPathInput.trim(), preferManaged: false },
                          })
                          pushToast(
                            t('settings.javaValid', {
                              version: result.version,
                              bits: result.bits,
                              vendor: result.vendor ?? 'Java',
                            }),
                          )
                        },
                        onError: (error) => {
                          pushToast(errorMessage(error, t('error.unknown')), 'error')
                        },
                      })
                    }}
                  >
                    {t('settings.javaValidate')}
                  </GlassButton>
                  {settings.java.customPath ? (
                    <GlassButton
                      size="sm"
                      variant="ghost"
                      onClick={() => {
                        customPathDirtyRef.current = false
                        setCustomPathInput('')
                        save({ java: { customPath: null, preferManaged: true } })
                      }}
                    >
                      {t('settings.javaReset')}
                    </GlassButton>
                  ) : null}
                </div>
              </div>

              {is32Bit ? (
                <div className={styles.warningBox} data-testid="java-32bit-warning">
                  <AlertCircle size={15} aria-hidden />
                  <span>{t('settings.java32BitWarning')}</span>
                </div>
              ) : null}

              <div className={styles.divider} />

              <div className={styles.field}>
                <span className={styles.fieldLabel}>{t('settings.jvmPreset')}</span>
                <p className={styles.rowHint}>{t('settings.jvmPresetHint')}</p>
                <div className={styles.presetRow}>
                  {jvmPresets.map(({ preset, label }) => {
                    const isSelected = settings.java.jvmPreset === preset
                    return (
                      <button
                        key={preset}
                        type="button"
                        className={`${styles.presetChip} ${isSelected ? styles.presetChipActive : ''}`}
                        aria-pressed={isSelected}
                        data-testid={`jvm-preset-${preset}`}
                        onClick={() => save({ java: { jvmPreset: preset } })}
                      >
                        {label}
                      </button>
                    )
                  })}
                </div>
              </div>

              {settings.java.jvmPreset === 'custom' ? (
                <div className={styles.field}>
                  <span className={styles.fieldLabel}>{t('settings.customJvmArgs')}</span>
                  <p className={styles.rowHint}>{t('settings.customJvmArgsHint')}</p>
                  <input
                    type="text"
                    className={styles.textInput}
                    value={customJvmArgsInput}
                    placeholder="-XX:+UnlockExperimentalVMOptions ..."
                    onChange={(event) => {
                      customJvmArgsDirtyRef.current = true
                      setCustomJvmArgsInput(event.target.value)
                    }}
                    onBlur={() => {
                      customJvmArgsDirtyRef.current = false
                      save({ java: { customJvmArgs: customJvmArgsInput } })
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') {
                        customJvmArgsDirtyRef.current = false
                        save({ java: { customJvmArgs: customJvmArgsInput } })
                      }
                    }}
                  />
                </div>
              ) : null}
            </section>
          ) : null}

          {activeCategory === 'downloads' ? (
            <section className={styles.panel}>
              <div className={styles.panelHead}>
                <h2 className={styles.panelTitle}>{t('settings.downloads')}</h2>
                <p className={styles.panelHint}>{t('settings.parallelismHint')}</p>
              </div>

              <div className={styles.valueRow}>
                <span className={styles.fieldLabel}>{t('settings.parallelism')}</span>
                <span className={styles.value}>
                  {t('settings.parallelismValue', { count: parallelism })}
                </span>
              </div>
              <GlassSlider
                value={parallelism}
                min={PARALLEL_MIN}
                max={PARALLEL_MAX}
                step={1}
                label={t('settings.parallelism')}
                onValueChange={setParallelism}
                onValueCommit={(value) => save({ downloads: { parallelism: value } })}
              />

              <p className={styles.rowHint}>{t('settings.parallelismHint')}</p>
            </section>
          ) : null}

          {activeCategory === 'discord' ? (
            <section className={styles.panel}>
              <div className={styles.panelHead}>
                <h2 className={styles.panelTitle}>{t('settings.discord')}</h2>
                <p className={styles.panelHint}>{t('settings.discordHint')}</p>
              </div>

              <div className={styles.field}>
                <div className={styles.row}>
                  <div className={styles.rowInfo}>
                    <span className={styles.fieldLabel}>{t('settings.discordEnabled')}</span>
                    <p className={styles.rowHint}>{t('settings.discordEnabledHint')}</p>
                  </div>
                  <GlassSwitch
                    checked={settings.discord.enabled}
                    label={t('settings.discordEnabled')}
                    onCheckedChange={(checked) => save({ discord: { enabled: checked } })}
                  />
                </div>
              </div>

              {settings.discord.enabled ? (
                <>
                  <div className={styles.divider} />

                  <div className={styles.field}>
                    <div className={styles.row}>
                      <div className={styles.rowInfo}>
                        <span className={styles.fieldLabel}>{t('settings.discordShowIdle')}</span>
                        <p className={styles.rowHint}>{t('settings.discordShowIdleHint')}</p>
                      </div>
                      <GlassSwitch
                        checked={settings.discord.showIdle}
                        label={t('settings.discordShowIdle')}
                        onCheckedChange={(checked) => save({ discord: { showIdle: checked } })}
                      />
                    </div>
                  </div>

                  <div className={styles.field}>
                    <div className={styles.row}>
                      <div className={styles.rowInfo}>
                        <span className={styles.fieldLabel}>{t('settings.discordShowPlaying')}</span>
                        <p className={styles.rowHint}>{t('settings.discordShowPlayingHint')}</p>
                      </div>
                      <GlassSwitch
                        checked={settings.discord.showPlaying}
                        label={t('settings.discordShowPlaying')}
                        onCheckedChange={(checked) => save({ discord: { showPlaying: checked } })}
                      />
                    </div>
                  </div>
                </>
              ) : null}
            </section>
          ) : null}
        </div>
      </div>
    </div>
  )
}
