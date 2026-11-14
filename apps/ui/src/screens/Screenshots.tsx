import type { ReactNode } from 'react'
import { Images, FolderOpen } from 'lucide-react'
import { errorMessage, invoke } from '../api/launcher'
import { useInstances } from '../api/queries'
import { useAppStore } from '../stores/app-store'
import styles from './Screenshots.module.css'

/** Galeri tangkapan layar per instance. */
export function Screenshots(): ReactNode {
  const instancesQuery = useInstances()
  const selectedInstanceId = useAppStore((state) => state.selectedInstanceId)
  const pushToast = useAppStore((state) => state.pushToast)

  const instances = instancesQuery.data ?? []
  const instance =
    instances.find((entry) => entry.id === selectedInstanceId) ?? instances[0] ?? null

  const openFolder = (): void => {
    if (!instance) return
    void invoke('instances:openFolder', { id: instance.id, subfolder: 'screenshots' }).catch(
      (error: unknown) => {
        pushToast(errorMessage(error, 'Could not open the screenshots folder.'), 'error')
      },
    )
  }

  return (
    <div className={styles.page} data-testid="screenshots-screen">
      <div className={styles.container}>
        <div className={styles.empty}>
          <div className={styles.artwork} aria-hidden>
            <Images size={68} strokeWidth={1.1} />
          </div>
          <h1 className={styles.title}>No screenshots yet</h1>
          <p className={styles.hint}>
            Screenshot browsing is not available yet. Your captures stay in the instance folder.
          </p>
          {instance ? (
            <>
              <button type="button" className={styles.action} onClick={openFolder}>
                <FolderOpen size={14} aria-hidden />
                <span>Open screenshots folder</span>
              </button>
              <p className={styles.instanceHint}>{instance.name}</p>
            </>
          ) : (
            <p className={styles.instanceHint}>
              Create an instance first to get a screenshots folder.
            </p>
          )}
        </div>
      </div>
    </div>
  )
}
