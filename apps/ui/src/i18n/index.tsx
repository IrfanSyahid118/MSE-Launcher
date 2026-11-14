import { createContext, useContext, useEffect, useMemo } from 'react'
import type { ReactNode } from 'react'
import type { Language } from '@mc/shared'
import { en } from './en'
import { id } from './id'
import type { MessageKey } from './id'

export type { MessageKey }

const dictionaries: Record<Language, Record<MessageKey, string>> = { id, en }

export type Translate = (key: MessageKey, variables?: Record<string, string | number>) => string

interface I18nValue {
  language: Language
  t: Translate
}

const I18nContext = createContext<I18nValue | null>(null)

function interpolate(template: string, variables?: Record<string, string | number>): string {
  if (!variables) {
    return template
  }
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = variables[name]
    return value === undefined ? match : String(value)
  })
}

export function I18nProvider({
  language,
  children,
}: {
  language: Language
  children: ReactNode
}): ReactNode {
  const value = useMemo<I18nValue>(() => {
    const dictionary = dictionaries[language]
    return {
      language,
      t: (key, variables) => interpolate(dictionary[key], variables),
    }
  }, [language])

  useEffect(() => {
    document.documentElement.lang = language
  }, [language])

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n(): I18nValue {
  const value = useContext(I18nContext)
  if (!value) {
    throw new Error('useI18n harus dipakai di dalam I18nProvider.')
  }
  return value
}
