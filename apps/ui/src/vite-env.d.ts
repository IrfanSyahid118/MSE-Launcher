import type { LauncherBridge } from '@mc/shared'

declare global {
  interface Window {
    launcher: LauncherBridge
  }
}
