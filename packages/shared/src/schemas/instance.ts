import { z } from 'zod'
import { jvmPresetSchema } from './settings'

export const loaderTypeSchema = z.enum(['vanilla', 'fabric', 'quilt', 'neoforge', 'forge'])

export const loaderSpecSchema = z.object({
  type: loaderTypeSchema,
  version: z.string().nullable(),
})

export const instanceJavaModeSchema = z.enum(['auto', 'custom'])

export const instanceJavaSettingsSchema = z.object({
  mode: instanceJavaModeSchema,
  path: z.string().nullable(),
})

export const instanceMemorySchema = z.object({
  overrideGlobal: z.boolean(),
  minMb: z.number().int().min(512).max(262_144).nullable(),
  maxMb: z.number().int().min(512).max(262_144).nullable(),
})

export const instanceJvmArgsSchema = z.object({
  preset: jvmPresetSchema,
  extra: z.array(z.string()),
})

export const contentTypeSchema = z.enum([
  'mod',
  'modpack',
  'shader',
  'resourcepack',
  'datapack',
  'world',
])

export const contentProviderIdSchema = z.enum(['modrinth', 'curseforge'])

export const instanceContentEntrySchema = z.object({
  provider: contentProviderIdSchema,
  projectId: z.string().min(1),
  versionId: z.string().min(1),
  type: contentTypeSchema,
  fileName: z.string().min(1),
  sha1: z.string().regex(/^([a-f0-9]{40})?$/i),
  worldFolder: z.string().min(1).max(128).optional(),
  disabled: z.boolean().optional(),
})

export const instanceSchema = z.object({
  schemaVersion: z.literal(1).default(1),
  id: z.string().min(1),
  name: z.string().min(1),
  /** Instance lama (folder `default` Tahap 1) bisa belum punya versi. */
  minecraftVersion: z.string(),
  loader: loaderSpecSchema,
  java: instanceJavaSettingsSchema,
  memory: instanceMemorySchema,
  jvmArgs: instanceJvmArgsSchema,
  content: z.array(instanceContentEntrySchema).default([]),
  createdAt: z.string().min(1),
  lastPlayedAt: z.string().min(1).nullable().default(null),
  iconUrl: z.string().nullable().optional(),
})

export const createInstanceRequestSchema = z.object({
  name: z.string().trim().min(1, 'Nama instance wajib diisi.').max(64),
  minecraftVersion: z.string().min(1, 'Versi Minecraft wajib dipilih.'),
  loader: loaderSpecSchema,
})
