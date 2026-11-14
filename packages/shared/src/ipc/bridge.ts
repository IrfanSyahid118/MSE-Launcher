import type { IpcEventChannel, IpcEventPayload } from './event-map'
import type { IpcInvokeChannel, IpcRequest, IpcResponse } from './invoke-map'
import type { IpcResult } from './result'

/**
 * Permukaan `window.launcher` yang diekspos preload lewat contextBridge.
 * Hasil IPC dikirim sebagai data biasa (IpcResult) supaya error tidak melewati
 * batas proses sebagai kelas kustom; lapisan `apps/ui/src/api` yang mengubahnya
 * menjadi `LauncherError`.
 */
export interface LauncherBridge {
  invoke<C extends IpcInvokeChannel>(
    channel: C,
    request: IpcRequest<C>,
  ): Promise<IpcResult<IpcResponse<C>>>
  on<E extends IpcEventChannel>(
    event: E,
    listener: (payload: IpcEventPayload<E>) => void,
  ): () => void
}
