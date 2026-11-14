import { useState } from 'react'
import type { ReactNode } from 'react'
import type { Instance } from '@mc/shared'
import { useContentProject } from '../../api/queries'
import defaultInstanceIcon from '../../assets/default-instance-icon.webp'
import styles from './InstanceIcon.module.css'

export interface InstanceIconProps {
  instance: Instance
  size?: number | undefined
  className?: string | undefined
  alt?: string | undefined
}

export function InstanceIcon({
  instance,
  size = 36,
  className,
  alt = '',
}: InstanceIconProps): ReactNode {
  const [iconBroken, setIconBroken] = useState(false)

  const modpack = instance.content?.find((entry) => entry.type === 'modpack') ?? null
  const projectQuery = useContentProject(
    modpack?.provider ?? 'modrinth',
    !instance.iconUrl ? (modpack?.projectId ?? null) : null,
  )

  const creatorIconUrl = instance.iconUrl || projectQuery.data?.iconUrl || null
  const isModpack = Boolean(instance.iconUrl || modpack)

  const activeSrc = isModpack && creatorIconUrl && !iconBroken ? creatorIconUrl : defaultInstanceIcon

  return (
    <span
      className={[styles.wrapper, className ?? ''].filter(Boolean).join(' ')}
      style={{ width: size, height: size, minWidth: size, minHeight: size }}
    >
      <img
        src={activeSrc}
        alt={alt}
        width={size}
        height={size}
        className={styles.image}
        loading="lazy"
        decoding="async"
        onError={() => {
          if (activeSrc !== defaultInstanceIcon) {
            setIconBroken(true)
          }
        }}
      />
    </span>
  )
}
