import { Switch } from 'radix-ui'
import type { ReactNode } from 'react'
import styles from './GlassSwitch.module.css'

export interface GlassSwitchProps {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  label: string
  disabled?: boolean
}

export function GlassSwitch({
  checked,
  onCheckedChange,
  label,
  disabled = false,
}: GlassSwitchProps): ReactNode {
  return (
    <Switch.Root
      className={styles.root}
      checked={checked}
      onCheckedChange={onCheckedChange}
      aria-label={label}
      disabled={disabled}
    >
      <Switch.Thumb className={styles.thumb} />
    </Switch.Root>
  )
}
