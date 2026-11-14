import { z } from 'zod'

export const accountTypeSchema = z.enum(['microsoft', 'offline'])

export const usernameSchema = z
  .string()
  .trim()
  .min(3, 'Username minimal 3 karakter.')
  .max(16, 'Username maksimal 16 karakter.')
  .regex(/^[A-Za-z0-9_]+$/, 'Username hanya boleh huruf, angka, dan garis bawah.')

export const accountSchema = z.object({
  id: z.uuid(),
  username: z.string().min(1),
  uuid: z.uuid(),
  type: accountTypeSchema,
  createdAt: z.string().min(1),
  expiresAt: z.string().min(1).nullable(),
  tokens: z.object({
    accessToken: z.string().min(1),
    refreshToken: z.string().min(1),
  }).optional(),
  xuid: z.string().min(1).optional(),
  skinUrl: z.string().min(1).optional(),
  skinVariant: z.enum(['classic', 'slim']).optional(),
  capeUrl: z.string().min(1).optional(),
})

export const accountsFileSchema = z.object({
  schemaVersion: z.literal(1),
  accounts: z.array(accountSchema),
})

export const createOfflineAccountSchema = z.object({ username: usernameSchema })

export const setActiveAccountSchema = z.object({ id: z.uuid() })

export const deviceCodeInfoSchema = z.object({
  deviceCode: z.string().min(1),
  userCode: z.string().min(1),
  verificationUri: z.string().url(),
  expiresIn: z.number().int().positive(),
  interval: z.number().int().positive(),
})
