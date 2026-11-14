import { z } from 'zod'

/**
 * Kunci API CurseForge diketik pengguna, jadi yang divalidasi hanya bentuk
 * kasarnya. Keabsahan kunci diuji dengan satu permintaan ke API saat disimpan.
 */
export const setCurseForgeKeySchema = z.object({
  apiKey: z.string().trim().max(200).nullable(),
})

export const setAzureClientIdSchema = z.object({
  clientId: z.string().trim().max(200).nullable(),
})
