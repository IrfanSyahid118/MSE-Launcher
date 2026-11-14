import { z } from 'zod'
import { contentProviderIdSchema, contentTypeSchema, loaderTypeSchema } from './instance'

const WINDOWS_RESERVED_NAME = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\..*)?$/i
const minecraftVersionSchema = z.string().regex(/^[A-Za-z0-9._-]+$/).min(1).max(64)
const projectIdSchema = z.string().trim().min(1).max(512)
const instanceIdSchema = z
  .string()
  .regex(/^[A-Za-z0-9._-]+$/)
  .min(1)
  .max(64)
  .refine(
    (value) => value !== '.' && value !== '..' && !WINDOWS_RESERVED_NAME.test(value),
    'ID instance tidak valid.',
  )

/** Nama folder world: tanpa pemisah path, bukan `.` atau `..` atau nama perangkat Windows. */
const worldFolderSchema = z
  .string()
  .min(1)
  .max(128)
  .refine(
    (value) =>
      value !== '.' &&
      value !== '..' &&
      !value.includes('/') &&
      !value.includes('\\') &&
      !WINDOWS_RESERVED_NAME.test(value),
    'Nama folder world tidak valid.',
  )

export const contentLoaderSchema = z.enum([
  ...loaderTypeSchema.options,
  'paper',
  'spigot',
  'bukkit',
  'purpur',
  'datapack',
])

export type ContentLoader = z.infer<typeof contentLoaderSchema>

export const searchQuerySchema = z.object({
  provider: contentProviderIdSchema,
  type: contentTypeSchema,
  text: z.string().max(200),
  page: z.number().int().min(1).max(500),
  pageSize: z.number().int().min(1).max(100),
  minecraftVersion: minecraftVersionSchema.optional(),
  loader: contentLoaderSchema.optional(),
  sort: z.enum(['relevance', 'downloads', 'follows', 'updated', 'newest']).optional(),
  categories: z.array(z.string().min(1).max(40)).max(10).optional(),
})

export const contentFilterSchema = z.object({
  minecraftVersion: minecraftVersionSchema.optional(),
  loader: contentLoaderSchema.optional(),
  featured: z.boolean().optional(),
})

export const contentVersionsRequestSchema = z.object({
  provider: contentProviderIdSchema,
  projectId: projectIdSchema,
  filter: contentFilterSchema,
})

export const contentProjectRequestSchema = z.object({
  provider: contentProviderIdSchema,
  projectId: projectIdSchema,
})

export const contentInstallRequestSchema = z.object({
  instanceId: instanceIdSchema,
  provider: contentProviderIdSchema,
  versionId: z.string().min(1).max(64),
  worldId: worldFolderSchema.optional(),
})

export const contentRemoveRequestSchema = z.object({
  instanceId: instanceIdSchema,
  provider: contentProviderIdSchema,
  projectId: projectIdSchema,
})

export const contentToggleRequestSchema = z.object({
  instanceId: instanceIdSchema,
  provider: contentProviderIdSchema,
  projectId: projectIdSchema,
  enabled: z.boolean(),
})

export const contentWorldsRequestSchema = z.object({
  instanceId: instanceIdSchema,
})

export const installModpackRequestSchema = z.object({
  provider: contentProviderIdSchema,
  versionId: z.string().min(1).max(64),
  instanceName: z.string().trim().min(1).max(64).optional(),
})

export const contentCheckUpdatesRequestSchema = z.object({
  instanceId: instanceIdSchema,
})

