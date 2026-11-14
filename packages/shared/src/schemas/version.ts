import { z } from 'zod'

export const minecraftVersionTypeSchema = z.enum(['release', 'snapshot', 'old_beta', 'old_alpha'])

export const mojangVersionEntrySchema = z.looseObject({
  id: z.string().min(1),
  type: minecraftVersionTypeSchema,
  url: z.url(),
  time: z.string().min(1),
  releaseTime: z.string().min(1),
  sha1: z.string().min(1).optional(),
  complianceLevel: z.number().optional(),
})

export const mojangVersionListSchema = z.looseObject({
  latest: z.looseObject({
    release: z.string().min(1),
    snapshot: z.string().min(1),
  }),
  versions: z.array(mojangVersionEntrySchema).min(1),
})

export const downloadArtifactSchema = z.looseObject({
  url: z.url(),
  sha1: z.string().min(1),
  size: z.number().int().nonnegative(),
})

export const libraryArtifactSchema = downloadArtifactSchema.extend({
  path: z.string().min(1),
})

export const assetIndexRefSchema = z.looseObject({
  id: z.string().min(1),
  sha1: z.string().min(1),
  size: z.number().int().nonnegative(),
  totalSize: z.number().int().nonnegative(),
  url: z.url(),
})

export const javaVersionRefSchema = z.looseObject({
  component: z.string().min(1),
  majorVersion: z.number().int().positive(),
})

export const versionLibrarySchema = z.looseObject({
  name: z.string().min(1),
  url: z.url().optional(),
  downloads: z
    .object({
      artifact: libraryArtifactSchema.optional(),
      classifiers: z.record(z.string(), libraryArtifactSchema).optional(),
    })
    .optional(),
  /** Checksum level-atas untuk library gaya Maven (profil loader). */
  sha1: z.string().min(1).optional(),
  /** Ukuran level-atas; tidak selalu disediakan meta loader. */
  size: z.number().int().nonnegative().optional(),
  rules: z.array(z.unknown()).optional(),
  natives: z.record(z.string(), z.string()).optional(),
  extract: z.looseObject({ exclude: z.array(z.string()).optional() }).optional(),
})

export const mojangVersionDetailSchema = z.looseObject({
  id: z.string().min(1),
  type: minecraftVersionTypeSchema,
  releaseTime: z.string().min(1),
  mainClass: z.string().min(1),
  assets: z.string().min(1),
  assetIndex: assetIndexRefSchema,
  downloads: z.looseObject({
    client: downloadArtifactSchema,
    server: downloadArtifactSchema.optional(),
  }),
  javaVersion: javaVersionRefSchema.optional(),
  libraries: z.array(versionLibrarySchema),
  arguments: z
    .looseObject({
      game: z.array(z.unknown()),
      jvm: z.array(z.unknown()),
    })
    .optional(),
  minecraftArguments: z.string().optional(),
  inheritsFrom: z.string().optional(),
  logging: z.unknown().optional(),
})

/**
 * Profil loader (Fabric/Quilt) bukan version JSON Mojang: tidak punya
 * `assetIndex`, `downloads`, atau `javaVersion`. Field itu diwarisi dari
 * `inheritsFrom` saat digabung.
 */
export const loaderProfileSchema = z.looseObject({
  id: z.string().min(1),
  inheritsFrom: z.string().min(1),
  type: z.string().optional(),
  releaseTime: z.string().optional(),
  mainClass: z.string().optional(),
  libraries: z.array(versionLibrarySchema).default([]),
  arguments: z
    .looseObject({
      game: z.array(z.unknown()).optional(),
      jvm: z.array(z.unknown()).optional(),
    })
    .optional(),
  minecraftArguments: z.string().optional(),
})

export type MojangVersionEntry = z.infer<typeof mojangVersionEntrySchema>
export type MojangVersionList = z.infer<typeof mojangVersionListSchema>
export type MojangVersionDetail = z.infer<typeof mojangVersionDetailSchema>
export type DownloadArtifact = z.infer<typeof downloadArtifactSchema>
export type VersionLibrary = z.infer<typeof versionLibrarySchema>
export type LoaderProfile = z.infer<typeof loaderProfileSchema>
