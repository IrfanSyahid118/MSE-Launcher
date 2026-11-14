import type { ButtonHTMLAttributes, ReactNode } from 'react'
import styles from './GlassButton.module.css'

export type GlassButtonVariant = 'primary' | 'ghost' | 'outline' | 'danger'
export type GlassButtonSize = 'sm' | 'md' | 'lg'

export interface GlassButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: GlassButtonVariant
  size?: GlassButtonSize
  icon?: ReactNode
}

export function GlassButton({
  variant = 'ghost',
  size = 'md',
  icon,
  children,
  className,
  type = 'button',
  ...rest
}: GlassButtonProps): ReactNode {
  return (
    <button
      type={type}
      className={[styles.button, styles[variant], styles[size], className ?? ''].filter(Boolean).join(' ')}
      {...rest}
    >
      {icon}
      {children}
    </button>
  )
}
