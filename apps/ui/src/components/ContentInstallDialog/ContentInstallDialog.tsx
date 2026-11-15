import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { AlertTriangle, Check, ExternalLink } from 'lucide-react'
import { compareMinecraftVersions, isVersionCompatibleWithMinecraft } from '@mc/shared'
import type { ContentProjectSummary, Instance, LoaderType } from '@mc/shared'
import { useI18n } from '../../i18n'
import {
  useContentPlan,
  useContentVersions,
  useContentWorlds,
  useInstallContent,
  useInstallModpack,
  useInstances,
} from '../../api/queries'
import { errorMessage, invoke } from '../../api/launcher'
import { registerInstallTask } from '../../api/task-registration'
import { useAppStore } from '../../stores/app-store'
import { GlassButton } from '../GlassButton/GlassButton'
import { GlassDialog } from '../GlassDialog/GlassDialog'
import { GlassSelect } from '../GlassSelect/GlassSelect'
import { VersionChannelFilter } from '../VersionChannelFilter/VersionChannelFilter'
import styles from './ContentInstallDialog.module.css'

export interface ContentInstallDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  project: ContentProjectSummary
  instance: Instance | null
  initialVersionId?: string | null | undefined
}

const EMPTY_VERSIONS = 0

/**
 * Pratinjau pemasangan: pilih versi yang cocok dengan instance, lihat
 * dependensi wajibnya, lalu unduh. Datapack membutuhkan world tujuan.
 */
export function ContentInstallDialog({
  open,
  onOpenChange,
  project,
  instance,
  initialVersionId = null,
}: ContentInstallDialogProps): ReactNode {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const pushToast = useAppStore((state) => state.pushToast)
  const [selectedVersionId, setSelectedVersionId] = useState<string | null>(initialVersionId)
  const [hideRelease, setHideRelease] = useState(false)
  const [hideBeta, setHideBeta] = useState(false)
  const [hideAlpha, setHideAlpha] = useState(false)
  const [modpackInstanceName, setModpackInstanceName] = useState(project.title)
  const [worldId, setWorldId] = useState('')
  const instancesQuery = useInstances()
  const instances = instancesQuery.data ?? []
  const [targetInstanceId, setTargetInstanceId] = useState<string>(instance?.id ?? '')
  const currentInstance = useMemo(
    () => instances.find((i) => i.id === targetInstanceId) ?? instance ?? instances[0] ?? null,
    [instances, targetInstanceId, instance],
  )
  const install = useInstallContent()
  const installModpack = useInstallModpack()
  const isModpack = project.type === 'modpack'
  const isPending = install.isPending || installModpack.isPending

  const filter = useMemo(
    () =>
      isModpack || !currentInstance
        ? {}
        : {
            minecraftVersion: currentInstance.minecraftVersion,
            ...(project.type === 'mod' && currentInstance.loader.type !== 'vanilla'
              ? { loader: currentInstance.loader.type }
              : project.type === 'datapack'
                ? { loader: 'datapack' as LoaderType }
                : {}),
          },
    [currentInstance, isModpack, project.type],
  )

  const versionsQuery = useContentVersions(project.provider, project.id, filter, open)
  const versions = useMemo(() => {
    const raw = versionsQuery.data ?? []
    const filtered = raw.filter((v) => {
      if (hideRelease && v.versionType === 'release') return false
      if (hideBeta && v.versionType === 'beta') return false
      if (hideAlpha && v.versionType === 'alpha') return false

      if (currentInstance && !isModpack) {
        if (
          !isVersionCompatibleWithMinecraft(
            v.gameVersions,
            currentInstance.minecraftVersion,
            v.versionNumber,
            v.name,
          )
        ) {
          return false
        }

        if (project.type === 'datapack') {
          if (v.loaders.length > 0 && !v.loaders.includes('datapack')) {
            return false
          }
        } else if (project.type === 'mod' && currentInstance.loader.type !== 'vanilla') {
          if (v.loaders.length > 0 && !v.loaders.includes(currentInstance.loader.type)) {
            return false
          }
        }
      }

      return true
    })
    return filtered.slice().sort((a, b) => {
      const typeRank: Record<string, number> = { release: 3, beta: 2, alpha: 1 }
      const rankDiff = (typeRank[b.versionType] ?? 0) - (typeRank[a.versionType] ?? 0)
      if (rankDiff !== 0) return rankDiff

      const timeA = Date.parse(a.publishedAt)
      const timeB = Date.parse(b.publishedAt)
      if (!Number.isNaN(timeA) && !Number.isNaN(timeB) && timeA !== timeB) {
        return timeB - timeA
      }

      return compareMinecraftVersions(a.versionNumber, b.versionNumber)
    })
  }, [
    versionsQuery.data,
    hideRelease,
    hideBeta,
    hideAlpha,
    currentInstance,
    isModpack,
    project.type,
  ])
  const isDatapack = project.type === 'datapack'
  const worldsQuery = useContentWorlds(currentInstance?.id ?? '', open && isDatapack && Boolean(currentInstance))
  const worlds = worldsQuery.data ?? []

  useEffect(() => {
    setSelectedVersionId(initialVersionId)
  }, [initialVersionId, project.id])

  useEffect(() => {
    if (!open || versions.length === EMPTY_VERSIONS || selectedVersionId !== null) {
      return
    }
    setSelectedVersionId(versions[0]?.id ?? null)
  }, [open, selectedVersionId, versions])

  const planQuery = useContentPlan(
    open && !isModpack && currentInstance ? currentInstance.id : null,
    project.provider,
    open && !isModpack ? selectedVersionId : null,
    isDatapack && worldId ? worldId : null,
  )
  const plan = planQuery.data

  const blocked = isDatapack && worldId.length === 0

  const handleInstall = (): void => {
    if (!selectedVersionId || blocked) {
      return
    }
    if (isModpack) {
      installModpack.mutate(
        {
          provider: project.provider,
          versionId: selectedVersionId,
          instanceName: modpackInstanceName.trim() || project.title,
          projectId: project.id,
          iconUrl: project.iconUrl,
        },
        {
          onSuccess: ({ taskId: id }) => {
            const outcome = registerInstallTask(id, project.title, queryClient, t)
            if (outcome === 'active') {
              pushToast(t('browse.installingItem', { title: project.title }), 'info')
            }
            onOpenChange(false)
          },
          onError: (err) => {
            pushToast(errorMessage(err, t('error.unknown')), 'error')
          },
        },
      )
      return
    }
    if (!currentInstance) {
      return
    }
    install.mutate(
      {
        instanceId: currentInstance.id,
        provider: project.provider,
        versionId: selectedVersionId,
        ...(isDatapack ? { worldId } : {}),
      },
      {
        onSuccess: ({ taskId: id }) => {
          const outcome = registerInstallTask(id, project.title, queryClient, t)
          if (outcome === 'active') {
            pushToast(t('browse.installingItem', { title: project.title }), 'info')
          }
          onOpenChange(false)
        },
        onError: (err) => {
          pushToast(errorMessage(err, t('error.unknown')), 'error')
        },
      },
    )
  }

  const target = plan?.target.version
  const dependencies = plan?.dependencies ?? []
  const isTargetBlocked = plan?.target && !plan.target.downloadable
  const blockedDependencies = dependencies.filter((d) => !d.downloadable)
  const isDistributionBlocked = Boolean(isTargetBlocked || blockedDependencies.length > 0)
  const isWorld = project.type === 'world'

  const curseForgeUrl =
    project.provider === 'curseforge'
      ? `https://www.curseforge.com/projects/${project.id}`
      : null

  return (
    <GlassDialog
      open={open}
      onOpenChange={onOpenChange}
      title={
        isModpack
          ? t('browse.installModpackTitle', { title: project.title })
          : t('browse.installTitle', { title: project.title })
      }
      description={
        isModpack
          ? t('browse.installModpackDescription')
          : t('browse.installTarget', {
              name: instance?.name ?? '',
              version: instance?.minecraftVersion || t('instance.versionUnavailable'),
            })
      }
      closeLabel={t('common.close')}
      size="md"
      footer={
        <>
          <GlassButton
            variant="ghost"
            onClick={() => onOpenChange(false)}
          >
            {t('common.cancel')}
          </GlassButton>
          {curseForgeUrl && isDistributionBlocked ? (
            <GlassButton
              variant="outline"
              icon={<ExternalLink size={14} aria-hidden />}
              onClick={() => {
                void invoke('shell:openExternal', { url: curseForgeUrl })
              }}
              data-testid="content-open-curseforge"
            >
              {t('browse.openInCurseForge')}
            </GlassButton>
          ) : null}
          <GlassButton
            variant="primary"
            icon={<Check size={14} aria-hidden />}
            disabled={
              isPending ||
              blocked ||
              !selectedVersionId ||
              (!isModpack && planQuery.isLoading) ||
              (!isModpack && isDistributionBlocked)
            }
            onClick={handleInstall}
            data-testid="content-install-confirm"
          >
            {isPending
              ? t('browse.installing')
              : isModpack
                ? t('browse.installModpackConfirm')
                : t('browse.installConfirm')}
          </GlassButton>
        </>
      }
    >
      <div className={styles.body} data-testid="content-install-dialog">
        {isModpack ? (
          <>
            <div className={styles.modpackDialogHeader}>
              {project.iconUrl ? (
                <img
                  src={project.iconUrl}
                  alt=""
                  className={styles.modpackDialogIcon}
                  width={44}
                  height={44}
                />
              ) : null}
              <div className={styles.modpackDialogInfo}>
                <span className={styles.modpackDialogTitle}>{project.title}</span>
                <span className={styles.modpackDialogProvider}>
                  {project.provider} · Modpack
                </span>
              </div>
            </div>

            <div className={styles.field}>
              <label htmlFor="modpack-name-input" className={styles.label}>
                {t('browse.modpackInstanceName')}
              </label>
              <input
                id="modpack-name-input"
                type="text"
                className={styles.input}
                value={modpackInstanceName}
                disabled={isPending}
                onChange={(e) => setModpackInstanceName(e.target.value)}
                data-testid="modpack-name-input"
              />
              <p className={styles.hint}>{t('browse.modpackInstanceNameHint')}</p>
            </div>
          </>
        ) : null}

        {!isModpack ? (
          <div className={styles.field}>
            <label htmlFor="content-target-instance" className={styles.label}>
              {t('browse.target')}
            </label>
            <select
              id="content-target-instance"
              className={styles.select}
              value={currentInstance?.id ?? ''}
              disabled={isPending}
              onChange={(e) => {
                setTargetInstanceId(e.target.value)
                setSelectedVersionId(null)
              }}
              data-testid="content-target-instance-select"
            >
              {instances.map((inst) => (
                <option key={inst.id} value={inst.id}>
                  {inst.name} ({inst.minecraftVersion} · {inst.loader.type})
                </option>
              ))}
            </select>
            {currentInstance ? (
              <p className={styles.hint}>
                {t('browse.installTarget', {
                  name: currentInstance.name,
                  version: currentInstance.minecraftVersion,
                })}
              </p>
            ) : null}
          </div>
        ) : null}

        <div className={styles.field}>
          <label htmlFor="content-version-select" className={styles.label}>
            {t('browse.version')}
          </label>
          <VersionChannelFilter
            hideRelease={hideRelease}
            hideBeta={hideBeta}
            hideAlpha={hideAlpha}
            onToggleRelease={setHideRelease}
            onToggleBeta={setHideBeta}
            onToggleAlpha={setHideAlpha}
          />
          {versionsQuery.isLoading ? (
            <p className={styles.hint}>{t('common.loading')}</p>
          ) : versionsQuery.isError ? (
            <p className={styles.error} role="alert">
              {errorMessage(versionsQuery.error, t('browse.versionsFailed'))}
            </p>
          ) : versions.length === 0 ? (
            <p className={styles.hint}>{t('browse.noCompatibleVersions')}</p>
          ) : (
            <GlassSelect
              id="content-version-select"
              value={selectedVersionId ?? ''}
              disabled={isPending}
              onValueChange={(val) => setSelectedVersionId(val)}
              options={versions.map((v) => ({
                value: v.id,
                label: v.versionNumber,
                badge: v.versionType.toUpperCase(),
              }))}
              placeholder={t('browse.version')}
              searchable={versions.length > 5}
              searchPlaceholder={t('instance.searchVersions')}
              testId="content-version-select"
            />
          )}
        </div>

        {isDatapack ? (
          <div className={styles.field}>
            <label htmlFor="content-world-select" className={styles.label}>
              {t('browse.world')}
            </label>
            {worlds.length === 0 ? (
              <p className={styles.hint}>{t('browse.noWorlds')}</p>
            ) : (
              <select
                id="content-world-select"
                className={styles.select}
                value={worldId}
                disabled={isPending}
                onChange={(e) => setWorldId(e.target.value)}
                data-testid="content-world-select"
              >
                <option value="">{t('browse.worldPlaceholder')}</option>
                {worlds.map((world) => (
                  <option key={world.folder} value={world.folder}>
                    {world.name}
                  </option>
                ))}
              </select>
            )}
          </div>
        ) : null}

        {!isModpack ? (
          <div className={styles.field}>
            <span className={styles.label}>{t('browse.plan')}</span>
            {planQuery.isLoading ? (
              <p className={styles.hint}>{t('common.loading')}</p>
            ) : planQuery.isError ? (
              <p className={styles.error} role="alert">
                {errorMessage(planQuery.error, t('browse.planFailed'))}
              </p>
            ) : plan && target ? (
              <div className={styles.plan} data-testid="content-install-plan">
                <p className={styles.planTarget}>
                  {target.fileName}
                  <span className={styles.planSize}>{target.versionNumber}</span>
                </p>
                {dependencies.length > 0 ? (
                  <div className={styles.planGroup}>
                    <span className={styles.planLabel}>
                      {t('browse.dependencies', { count: dependencies.length })}
                    </span>
                    <ul className={styles.planList}>
                      {dependencies.map((dependency) => (
                        <li key={dependency.version.id} className={styles.planItem}>
                          {dependency.version.fileName}
                          <span className={styles.planSize}>{dependency.type}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {plan.warnings.length > 0 ? (
                  <ul className={styles.warnings} data-testid="content-install-warnings">
                    {plan.warnings.map((warning) => (
                      <li key={warning} className={styles.warning}>
                        <AlertTriangle size={13} aria-hidden />
                        {warning}
                      </li>
                    ))}
                  </ul>
                ) : null}
                {isDistributionBlocked ? (
                  <div className={styles.blockedNotice} data-testid="content-distribution-blocked">
                    <AlertTriangle size={14} aria-hidden />
                    <div>
                      <p className={styles.blockedTitle}>{t('browse.blockedHint')}</p>
                      {curseForgeUrl ? (
                        <GlassButton
                          size="sm"
                          variant="outline"
                          icon={<ExternalLink size={12} aria-hidden />}
                          onClick={() => {
                            void invoke('shell:openExternal', { url: curseForgeUrl })
                          }}
                        >
                          {t('browse.openInCurseForge')}
                        </GlassButton>
                      ) : null}
                    </div>
                  </div>
                ) : null}
                {isWorld ? (
                  <p className={styles.worldNote} data-testid="content-world-note">
                    {t('browse.worldInstallNote')}
                  </p>
                ) : null}
              </div>
            ) : (
              <p className={styles.hint}>{t('browse.planPending')}</p>
            )}
          </div>
        ) : null}

        {isPending ? (
          <p className={styles.installingNote}>
            {t('browse.installing')}
          </p>
        ) : null}
      </div>
    </GlassDialog>
  )
}
