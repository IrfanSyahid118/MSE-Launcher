import { LauncherError } from '@mc/shared'
import type { IpcInvokeChannel, IpcInvokeMap } from '@mc/shared'

/**
 * Satu-satunya jalur UI ke sistem. Membuka bungkus IpcResult dan mengubah
 * data error menjadi LauncherError supaya komponen bisa menampilkan pesan siap
 * pakai tanpa stack trace.
 */
export async function invoke<C extends IpcInvokeChannel>(
  channel: C,
  request: IpcInvokeMap[C]['req'],
): Promise<IpcInvokeMap[C]['res']> {
  const result = await window.launcher.invoke(channel, request)
  if (result.ok) {
    return result.data
  }
  throw new LauncherError(result.error.code, result.error.message, {
    retryable: result.error.retryable,
    ...(result.error.details === undefined ? {} : { details: result.error.details }),
  })
}

export function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof LauncherError) {
    return error.message
  }
  if (error instanceof Error && error.message) {
    return error.message
  }
  return fallback
}
