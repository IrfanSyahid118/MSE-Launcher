import { useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { Check, ChevronDown, ChevronUp, Search, X } from 'lucide-react'
import { Select } from 'radix-ui'
import { useI18n } from '../../i18n'
import styles from './GlassSelect.module.css'

export interface GlassSelectOption {
  value: string
  label: string
  badge?: string
  icon?: ReactNode
  disabled?: boolean
}

export interface GlassSelectProps {
  id?: string
  value: string
  onValueChange: (value: string) => void
  options: GlassSelectOption[]
  placeholder?: string
  disabled?: boolean
  searchable?: boolean
  searchPlaceholder?: string
  className?: string
  testId?: string
}

export function GlassSelect({
  id,
  value,
  onValueChange,
  options,
  placeholder,
  disabled,
  searchable,
  searchPlaceholder,
  className,
  testId,
}: GlassSelectProps): ReactNode {
  const { t } = useI18n()
  const [search, setSearch] = useState('')

  const filteredOptions = useMemo(() => {
    if (!searchable || !search.trim()) return options
    const q = search.trim().toLowerCase()
    return options.filter(
      (opt) =>
        opt.label.toLowerCase().includes(q) ||
        (opt.badge && opt.badge.toLowerCase().includes(q)),
    )
  }, [options, search, searchable])

  const selectedOption = useMemo(
    () => options.find((opt) => opt.value === value),
    [options, value],
  )

  const getBadgeClass = (badge?: string): string => {
    if (!badge) return ''
    const b = badge.toLowerCase()
    if (b.includes('beta')) return styles.badgeBeta ?? ''
    if (b.includes('alpha')) return styles.badgeAlpha ?? ''
    return styles.badgeRelease ?? ''
  }

  return (
    <Select.Root
      value={value}
      onValueChange={onValueChange}
      disabled={disabled}
      onOpenChange={(open) => {
        if (!open) setSearch('')
      }}
    >
      <Select.Trigger
        id={id}
        className={`${styles.trigger} ${className || ''}`}
        data-testid={testId}
        aria-label={selectedOption?.label || placeholder}
      >
        <div className={styles.triggerValue}>
          {selectedOption ? (
            <>
              {selectedOption.icon ? <span>{selectedOption.icon}</span> : null}
              <span>{selectedOption.label}</span>
              {selectedOption.badge ? (
                <span className={`${styles.badge} ${getBadgeClass(selectedOption.badge)}`}>
                  {selectedOption.badge}
                </span>
              ) : null}
            </>
          ) : (
            <span style={{ color: 'var(--text-tertiary)' }}>{placeholder}</span>
          )}
        </div>
        <Select.Icon className={styles.triggerIcon}>
          <ChevronDown size={14} aria-hidden />
        </Select.Icon>
      </Select.Trigger>

      <Select.Portal>
        <Select.Content
          className={styles.content}
          position="popper"
          sideOffset={6}
          align="start"
        >
          {searchable ? (
            <div
              className={styles.searchBox}
              onKeyDown={(e) => e.stopPropagation()}
            >
              <Search size={13} className={styles.searchIcon} aria-hidden />
              <input
                type="text"
                className={styles.searchInput}
                placeholder={searchPlaceholder ?? t('common.search')}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                autoFocus
              />
              {search ? (
                <button
                  type="button"
                  className={styles.searchClear}
                  onClick={() => setSearch('')}
                >
                  <X size={12} aria-hidden />
                </button>
              ) : null}
            </div>
          ) : null}

          <Select.ScrollUpButton className={styles.scrollButton}>
            <ChevronUp size={14} aria-hidden />
          </Select.ScrollUpButton>

          <Select.Viewport className={styles.viewport}>
            {filteredOptions.length > 0 ? (
              filteredOptions.map((opt) => (
                <Select.Item
                  key={opt.value}
                  value={opt.value}
                  className={styles.item}
                  disabled={opt.disabled}
                >
                  <div className={styles.itemMain}>
                    {opt.icon ? <span>{opt.icon}</span> : null}
                    <Select.ItemText>{opt.label}</Select.ItemText>
                    {opt.badge ? (
                      <span className={`${styles.badge} ${getBadgeClass(opt.badge)}`}>
                        {opt.badge}
                      </span>
                    ) : null}
                  </div>
                  <Select.ItemIndicator className={styles.itemIndicator}>
                    <Check size={14} aria-hidden />
                  </Select.ItemIndicator>
                </Select.Item>
              ))
            ) : (
              <div className={styles.emptyText}>{t('common.noResults')}</div>
            )}
          </Select.Viewport>

          <Select.ScrollDownButton className={styles.scrollButton}>
            <ChevronDown size={14} aria-hidden />
          </Select.ScrollDownButton>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  )
}
