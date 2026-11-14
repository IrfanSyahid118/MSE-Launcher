import type { LauncherErrorData } from '../errors'
import type { LogLine, ProgressEvent } from '../models/progress'
import type { IpcInvokeChannel } from './invoke-map'

export interface IpcEventMap {
  'task:progress': { taskId: string; progress: ProgressEvent }
  'task:done': { taskId: string; result?: unknown }
  'task:error': { taskId: string; error: LauncherErrorData }
  'game:log': { instanceId: string; lines: LogLine[] }
  'game:exit': { instanceId: string; code: number | null }
  'launcher:log': LogLine[]
}

export type IpcEventChannel = keyof IpcEventMap

export type IpcEventPayload<E extends IpcEventChannel> = IpcEventMap[E]

/** Nama pekerjaan yang ramah pengguna untuk dock progres, dari channel task-nya. */
export function installJobTitle(channel: IpcInvokeChannel): string {
  switch (channel) {
    case 'content:install':
      return 'Instalasi konten'
    case 'content:installModpack':
      return 'Instalasi modpack'
    case 'java:ensure':
      return 'Unduh Java'
    case 'launch:start':
      return 'Menyiapkan game'
    default:
      return 'Unduhan'
  }
}
