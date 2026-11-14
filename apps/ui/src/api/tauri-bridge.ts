import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen as tauriListen } from '@tauri-apps/api/event'

function channelToCommand(channel: string): string {
  return channel
    .replace(':', '_')
    .replace(/([A-Z])/g, '_$1')
    .toLowerCase()
}

interface TauriErrorPayload {
  code?: string
  message?: string
  retryable?: boolean
}

export function initTauriBridge(): void {
  const isTauri =
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)

  if (isTauri || !window.launcher) {
    window.launcher = {
      invoke: async (channel: string, request: unknown) => {
        try {
          const commandName = channelToCommand(channel)
          let args: Record<string, unknown> = {}
          if (request !== null && typeof request === 'object' && !Array.isArray(request)) {
            args = {
              ...(request as Record<string, unknown>),
              req: request,
              patch: request,
              request,
            }
          } else if (request !== undefined && request !== null) {
            args = { request }
          }

          const data = await tauriInvoke(commandName, args)
          return { ok: true, data }
        } catch (err: unknown) {
          const tErr = err && typeof err === 'object' ? (err as TauriErrorPayload) : null
          return {
            ok: false,
            error: {
              code: tErr?.code ?? 'INTERNAL',
              message: tErr?.message ?? String(err),
              retryable: tErr?.retryable ?? false,
            },
          }
        }
      },
      on: (channel: string, callback: (payload: unknown) => void) => {
        let disposed = false
        let unlistenFn: (() => void) | null = null
        if (isTauri) {
          void tauriListen(channel, (event) => {
            if (!disposed) {
              callback(event.payload)
            }
          }).then((unlisten) => {
            if (disposed) {
              unlisten()
            } else {
              unlistenFn = unlisten
            }
          })
        }
        return () => {
          disposed = true
          if (unlistenFn) {
            unlistenFn()
            unlistenFn = null
          }
        }
      },
    } as unknown as typeof window.launcher
  }
}
