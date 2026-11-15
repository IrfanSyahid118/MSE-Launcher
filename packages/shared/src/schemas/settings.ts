import { z } from 'zod'
import { DEFAULT_DOWNLOADS, DEFAULT_JAVA, DEFAULT_MEMORY, DEFAULT_THEME } from '../models/settings'

export const themeModeSchema = z.enum(['system', 'dark', 'light'])
export const languageSchema = z.enum(['id', 'en'])

export const themeSettingsSchema = z.object({
  mode: themeModeSchema.default(DEFAULT_THEME.mode),
  reduceTransparency: z.boolean().default(DEFAULT_THEME.reduceTransparency),
  reduceMotion: z.boolean().default(DEFAULT_THEME.reduceMotion),
})

export const memorySettingsSchema = z
  .object({
    defaultMaxMb: z.number().int().min(512).max(262_144).default(DEFAULT_MEMORY.defaultMaxMb),
    defaultMinMb: z.number().int().min(512).max(262_144).default(DEFAULT_MEMORY.defaultMinMb),
  })
  .refine((value) => value.defaultMinMb <= value.defaultMaxMb, {
    message: 'RAM minimum tidak boleh melebihi RAM maksimum.',
    path: ['defaultMinMb'],
  })

export const jvmPresetSchema = z.enum(['default', 'performance', 'low-memory', 'custom'])

export const javaSettingsSchema = z.object({
  preferManaged: z.boolean().default(DEFAULT_JAVA.preferManaged),
  customPath: z.string().min(1).nullable().default(DEFAULT_JAVA.customPath),
  jvmPreset: jvmPresetSchema.default(DEFAULT_JAVA.jvmPreset),
  customJvmArgs: z.string().default(DEFAULT_JAVA.customJvmArgs),
})

export const downloadSettingsSchema = z.object({
  parallelism: z.number().int().min(1).max(16).default(DEFAULT_DOWNLOADS.parallelism),
})

export const discordSettingsSchema = z.object({
  enabled: z.boolean().default(true),
  showIdle: z.boolean().default(true),
  showPlaying: z.boolean().default(true),
  customClientId: z.string().nullable().default(null),
})

export const settingsSchema = z.object({
  schemaVersion: z.literal(1).default(1),
  theme: themeSettingsSchema.default(() => ({ ...DEFAULT_THEME })),
  memory: memorySettingsSchema.default(() => ({ ...DEFAULT_MEMORY })),
  java: javaSettingsSchema.default(() => ({ ...DEFAULT_JAVA })),
  downloads: downloadSettingsSchema.default(() => ({ ...DEFAULT_DOWNLOADS })),
  discord: discordSettingsSchema.default(() => ({
    enabled: true,
    showIdle: true,
    showPlaying: true,
    customClientId: null,
  })),
  language: languageSchema.default('id'),
  activeAccountId: z.uuid().nullable().default(null),
})

export const themePatchSchema = z.object({
  mode: themeModeSchema.optional(),
  reduceTransparency: z.boolean().optional(),
  reduceMotion: z.boolean().optional(),
})

export const memoryPatchSchema = z.object({
  defaultMaxMb: z.number().int().min(512).max(262_144).optional(),
  defaultMinMb: z.number().int().min(512).max(262_144).optional(),
})

export const discordPatchSchema = z.object({
  enabled: z.boolean().optional(),
  showIdle: z.boolean().optional(),
  showPlaying: z.boolean().optional(),
  customClientId: z.string().nullable().optional(),
})

export const settingsPatchSchema = z.object({
  theme: themePatchSchema.optional(),
  memory: memoryPatchSchema.optional(),
  java: javaSettingsSchema.partial().optional(),
  downloads: downloadSettingsSchema.partial().optional(),
  discord: discordPatchSchema.optional(),
  language: languageSchema.optional(),
  activeAccountId: z.uuid().nullable().optional(),
})

export type SettingsPatchInput = z.infer<typeof settingsPatchSchema>
