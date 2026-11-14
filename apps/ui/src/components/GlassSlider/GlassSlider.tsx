import { Slider } from 'radix-ui'
import type { ReactNode } from 'react'
import styles from './GlassSlider.module.css'

export interface GlassSliderProps {
  value: number
  min: number
  max: number
  step: number
  label: string
  onValueChange: (value: number) => void
  onValueCommit?: (value: number) => void
  disabled?: boolean
}

export function GlassSlider({
  value,
  min,
  max,
  step,
  label,
  onValueChange,
  onValueCommit,
  disabled = false,
}: GlassSliderProps): ReactNode {
  return (
    <Slider.Root
      className={styles.root}
      value={[value]}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      aria-label={label}
      onValueChange={(next) => {
        const first = next[0]
        if (first !== undefined) {
          onValueChange(first)
        }
      }}
      onValueCommit={(next) => {
        const first = next[0]
        if (first !== undefined) {
          onValueCommit?.(first)
        }
      }}
    >
      <Slider.Track className={styles.track}>
        <Slider.Range className={styles.range} />
      </Slider.Track>
      <Slider.Thumb className={styles.thumb}>
        <span className={styles.thumbHighlight} aria-hidden />
      </Slider.Thumb>
    </Slider.Root>
  )
}
