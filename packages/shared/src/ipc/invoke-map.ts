import type { Account } from '../models/account'
import type {
  ContentFilter,
  ContentInstallPlan,
  ContentProject,
  ContentProviderId,
  ContentUpdateInfo,
  ContentVersion,
  InstallModpackRequest,
  SavedWorld,
  SearchQuery,
  SearchResult,
} from '../models/content'
import type { CreateInstanceRequest, Instance, LoaderType } from '../models/instance'
import type { IntegrationsStatus } from '../models/integrations'
import type { DeviceCodeInfo, JavaInstallation } from '../models/java-auth'
import type { DiagnosticReportResult, LogLine } from '../models/progress'
import type {
  InstanceServerEntry,
  ServerPingStatus,
  ServerResourcePackPolicy,
} from '../models/server'
import type { Settings, SettingsPatch } from '../models/settings'
import type { MemoryInfo, UpdateCheckResult } from '../models/system'
import type { LoaderGameSupport, LoaderVersionInfo, MinecraftVersionInfo } from '../models/version'

/**
 * Kontrak channel request/response. Channel yang belum diimplementasikan pada
 * tahap berjalan tidak didaftarkan di main process (bukan stub) dan belum bisa
 * dipanggil dari renderer.
 */
export interface IpcInvokeMap {
  'system:memoryInfo': { req: void; res: MemoryInfo }
  'settings:get': { req: void; res: Settings }
  'settings:set': { req: SettingsPatch; res: Settings }
  'versions:list': { req: void; res: MinecraftVersionInfo[] }
  'loaders:versions': {
    req: { type: LoaderType; minecraftVersion: string }
    res: LoaderVersionInfo[]
  }
  'loaders:games': { req: void; res: LoaderGameSupport }
  'accounts:list': { req: void; res: Account[] }
  'accounts:setActive': { req: { id: string }; res: void }
  'accounts:updateSkin': {
    req: {
      accountId: string
      skinData: string
      skinVariant?: 'classic' | 'slim' | undefined
      capeData?: string | undefined
    }
    res: Account
  }
  'accounts:syncMinecraft': {
    req: { accountId: string }
    res: Account
  }
  'accounts:setActiveCape': {
    req: {
      accountId: string
      capeId?: string | undefined
      customCapeData?: string | undefined
    }
    res: Account
  }
  'accounts:installSkinResourcePack': {
    req: {
      accountId: string
      instanceIds: string[]
    }
    res: {
      updatedInstances: number
      packFileName: string
    }
  }
  'accounts:exportSkinResourcePack': {
    req: {
      accountId: string
      destinationPath: string
    }
    res: string
  }
  'auth:createOffline': { req: { username: string }; res: Account }
  'instances:list': { req: void; res: Instance[] }
  'instances:create': { req: CreateInstanceRequest; res: Instance }
  'instances:update': { req: Instance; res: void }
  'instances:remove': { req: { id: string }; res: void }
  'instances:openFolder': {
    req: { id: string; subfolder?: 'mods' | 'resourcepacks' | 'saves' | 'shaderpacks' | 'screenshots' | 'logs' | undefined }
    res: void
  }
  'downloads:cleanup': { req: void; res: { taskId: string } }
  'launch:start': {
    req: { instanceId: string; versionId: string; serverAddress?: string | undefined }
    res: { taskId: string }
  }
  'launch:kill': { req: { instanceId: string }; res: void }
  'auth:beginMicrosoft': { req: void; res: DeviceCodeInfo }
  'auth:completeMicrosoft': { req: DeviceCodeInfo; res: Account }
  'auth:signOut': { req: { accountId: string }; res: void }
  'java:detect': { req: void; res: JavaInstallation[] }
  'java:validate': { req: { path: string }; res: JavaInstallation }
  'java:ensure': { req: { major: number }; res: { taskId: string } }
  'content:search': { req: SearchQuery; res: SearchResult }
  'content:project': {
    req: { provider: ContentProviderId; projectId: string }
    res: ContentProject
  }
  'content:versions': {
    req: { provider: ContentProviderId; projectId: string; filter: ContentFilter }
    res: ContentVersion[]
  }
  'content:plan': {
    req: {
      instanceId: string
      provider: ContentProviderId
      versionId: string
      worldId?: string | undefined
    }
    res: ContentInstallPlan
  }
  'content:install': {
    req: {
      instanceId: string
      provider: ContentProviderId
      versionId: string
      worldId?: string | undefined
    }
    res: { taskId: string }
  }
  'content:remove': {
    req: { instanceId: string; provider: ContentProviderId; projectId: string }
    res: Instance
  }
  'content:toggle': {
    req: { instanceId: string; provider: ContentProviderId; projectId: string; enabled: boolean }
    res: Instance
  }
  'content:worlds': { req: { instanceId: string }; res: SavedWorld[] }
  'content:installModpack': {
    req: InstallModpackRequest
    res: { taskId: string }
  }
  'content:checkUpdates': {
    req: { instanceId: string }
    res: ContentUpdateInfo[]
  }
  'servers:list': { req: { instanceId: string }; res: InstanceServerEntry[] }
  'servers:add': {
    req: {
      instanceId: string
      name: string
      ip: string
      resourcePackPolicy?: ServerResourcePackPolicy | undefined
      hideFromHome?: boolean | undefined
    }
    res: InstanceServerEntry[]
  }
  'servers:remove': { req: { instanceId: string; ip: string }; res: InstanceServerEntry[] }
  'servers:edit': {
    req: {
      instanceId: string
      oldIp: string
      name: string
      newIp: string
      resourcePackPolicy?: ServerResourcePackPolicy | undefined
      hideFromHome?: boolean | undefined
    }
    res: InstanceServerEntry[]
  }
  'servers:ping': {
    req: { instanceId?: string | undefined; ip: string }
    res: ServerPingStatus
  }
  'integrations:status': { req: void; res: IntegrationsStatus }
  'integrations:setCurseForgeKey': {
    req: { apiKey: string | null }
    res: IntegrationsStatus
  }
  'integrations:setAzureClientId': {
    req: { clientId: string | null }
    res: IntegrationsStatus
  }
  'tasks:cancel': { req: { taskId: string }; res: void }
  'shell:openExternal': { req: { url: string }; res: void }
  'logs:launcherHistory': { req: void; res: LogLine[] }
  'logs:appendClient': { req: { stream: 'stdout' | 'stderr' | 'system'; line: string }; res: void }
  'logs:clearLauncher': { req: void; res: void }
  'logs:createDiagnostic': {
    req: {
      instanceId?: string | undefined
      gameLines?: LogLine[] | undefined
      lastError?: string | undefined
    }
    res: DiagnosticReportResult
  }
  'logs:openLauncherFolder': { req: void; res: void }
  'updater:check': { req: void; res: UpdateCheckResult }
  'updater:install': {
    req: {
      downloadUrl: string
      assetName: string
      sha256?: string | undefined
      sizeBytes?: number | undefined
    }
    res: { taskId: string }
  }
}

export type IpcInvokeChannel = keyof IpcInvokeMap

export type IpcRequest<C extends IpcInvokeChannel> = IpcInvokeMap[C]['req']

export type IpcResponse<C extends IpcInvokeChannel> = IpcInvokeMap[C]['res']
