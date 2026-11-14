import { z } from 'zod'

const WINDOWS_RESERVED_NAME = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\..*)?$/i
const instanceIdSchema = z
  .string()
  .regex(/^[A-Za-z0-9._-]+$/)
  .min(1)
  .max(64)
  .refine(
    (value) => value !== '.' && value !== '..' && !WINDOWS_RESERVED_NAME.test(value),
    'ID instance tidak valid.',
  )

export const serverResourcePackPolicySchema = z.enum(['prompt', 'enabled', 'disabled'])

export const instanceServerEntrySchema = z.object({
  name: z.string().trim().min(1).max(128),
  ip: z.string().trim().min(1).max(256),
  icon: z.string().nullable().optional(),
  acceptTextures: z.number().int().min(0).max(2).optional(),
  resourcePackPolicy: serverResourcePackPolicySchema.optional(),
  hideFromHome: z.boolean().optional(),
})

export const instanceServersListRequestSchema = z.object({
  instanceId: instanceIdSchema,
})

export const instanceServerAddRequestSchema = z.object({
  instanceId: instanceIdSchema,
  name: z.string().trim().min(1, 'Nama server tidak boleh kosong').max(128),
  ip: z.string().trim().min(1, 'Alamat server tidak boleh kosong').max(256),
  resourcePackPolicy: serverResourcePackPolicySchema.optional(),
  hideFromHome: z.boolean().optional(),
})

export const instanceServerRemoveRequestSchema = z.object({
  instanceId: instanceIdSchema,
  ip: z.string().trim().min(1).max(256),
})

export const instanceServerEditRequestSchema = z.object({
  instanceId: instanceIdSchema,
  oldIp: z.string().trim().min(1).max(256),
  name: z.string().trim().min(1, 'Nama server tidak boleh kosong').max(128),
  newIp: z.string().trim().min(1, 'Alamat server tidak boleh kosong').max(256),
  resourcePackPolicy: serverResourcePackPolicySchema.optional(),
  hideFromHome: z.boolean().optional(),
})

export const instanceServerPingRequestSchema = z.object({
  instanceId: instanceIdSchema.optional(),
  ip: z.string().trim().min(1).max(256),
})
