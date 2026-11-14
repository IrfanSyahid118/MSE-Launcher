import type { LauncherErrorData } from '../errors'

export type IpcResult<T> = { ok: true; data: T } | { ok: false; error: LauncherErrorData }

export function ipcOk<T>(data: T): IpcResult<T> {
  return { ok: true, data }
}

export function ipcError(error: LauncherErrorData): IpcResult<never> {
  return { ok: false, error }
}
