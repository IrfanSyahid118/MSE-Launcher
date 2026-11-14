import { useId, useState } from 'react'
import type { ReactNode } from 'react'
import { Check, ChevronDown } from 'lucide-react'
import { Popover } from 'radix-ui'
import type { LoaderGameSupport, LoaderType } from '@mc/shared'
import { useI18n } from '../../i18n'
import styles from './LoaderPicker.module.css'

const LOADER_OPTIONS: LoaderType[] = ['vanilla', 'fabric', 'quilt', 'forge', 'neoforge']

const LOADER_LABELS: Record<LoaderType, string> = {
  vanilla: 'Vanilla',
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
  neoforge: 'NeoForge',
}

export interface LoaderPickerProps {
  id: string
  value: LoaderType
  minecraftVersion: string
  support: LoaderGameSupport | undefined
  supportPending: boolean
  onChange: (type: LoaderType) => void
  testId: string
}

export type LoaderAvailability = 'available' | 'unavailable' | 'unknown' | 'checking'

/**
 * Hasil satu-satunya sumber untuk "boleh dipilih atau tidak". Daftar dukungan
 * yang belum termuat (null) atau masih dimuat tidak boleh dianggap didukung:
 * kalau dianggap didukung, UI memanggil `loaders:versions` untuk kombinasi yang
 * pasti gagal dan hanya menghasilkan spam error.
 */
export function loaderAvailability(
  type: LoaderType,
  minecraftVersion: string,
  support: LoaderGameSupport | undefined,
  supportPending: boolean,
): LoaderAvailability {
  if (type === 'vanilla') {
    return 'available'
  }
  if (!support) {
    return supportPending ? 'checking' : 'unknown'
  }
  const list = support[type]
  if (list === null) {
    return 'unknown'
  }
  return list.includes(minecraftVersion) ? 'available' : 'unavailable'
}

export function LoaderPicker({
  id,
  value,
  minecraftVersion,
  support,
  supportPending,
  onChange,
  testId,
}: LoaderPickerProps): ReactNode {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const listId = useId()
  const options = LOADER_OPTIONS.includes(value)
    ? LOADER_OPTIONS
    : [...LOADER_OPTIONS, value]

  const availability = loaderAvailability(value, minecraftVersion, support, supportPending)
  const triggerStatus =
    availability === 'unavailable'
      ? t('instance.loaderStatusUnavailable')
      : availability === 'unknown'
        ? t('instance.loaderStatusUnknown')
        : availability === 'checking'
          ? t('instance.loaderStatusChecking')
          : null

  const choose = (type: LoaderType): void => {
    if (loaderAvailability(type, minecraftVersion, support, supportPending) !== 'available') {
      return
    }
    onChange(type)
    setOpen(false)
  }

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          id={id}
          className={styles.trigger}
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-controls={open ? listId : undefined}
          data-testid={`${testId}-trigger`}
        >
          <span className={styles.triggerName}>{LOADER_LABELS[value]}</span>
          {triggerStatus ? (
            <span className={styles.triggerStatus}>{triggerStatus}</span>
          ) : null}
          <ChevronDown size={16} aria-hidden className={styles.chevron} />
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className={styles.popover}
          align="start"
          sideOffset={6}
          collisionPadding={16}
          onOpenAutoFocus={(event) => event.preventDefault()}
        >
          <div
            className={styles.panel}
            id={listId}
            role="group"
            aria-label={t('instance.loaderChoose')}
          >
            {options.map((type) => {
              const optionAvailability = loaderAvailability(
                type,
                minecraftVersion,
                support,
                supportPending,
              )
              const selected = type === value
              const status =
                optionAvailability === 'unavailable'
                  ? t('instance.loaderStatusUnavailable')
                  : optionAvailability === 'unknown'
                    ? t('instance.loaderStatusUnknown')
                    : optionAvailability === 'checking'
                      ? t('instance.loaderStatusChecking')
                      : null
              return (
                <button
                  key={type}
                  type="button"
                  className={styles.option}
                  aria-pressed={selected}
                  aria-disabled={optionAvailability !== 'available' || undefined}
                  onClick={() => choose(type)}
                  data-testid={`${testId}-option-${type}`}
                >
                  <span className={styles.name}>{LOADER_LABELS[type]}</span>
                  {status ? (
                    <span
                      className={
                        optionAvailability === 'unavailable'
                          ? `${styles.status} ${styles.statusUnavailable}`
                          : styles.status
                      }
                    >
                      {status}
                    </span>
                  ) : null}
                  {selected ? (
                    <span className={styles.check}>
                      <Check size={14} aria-hidden />
                      <span className={styles.srOnly}>{t('version.selected')}</span>
                    </span>
                  ) : null}
                </button>
              )
            })}
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
