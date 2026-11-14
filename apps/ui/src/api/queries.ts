import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type {
  Account,
  ContentFilter,
  ContentInstallPlan,
  ContentProject,
  ContentProviderId,
  ContentUpdateInfo,
  ContentVersion,
  CreateInstanceRequest,
  DeviceCodeInfo,
  InstallModpackRequest,
  Instance,
  InstanceServerEntry,
  IntegrationsStatus,
  JavaInstallation,
  LoaderGameSupport,
  LoaderType,
  LoaderVersionInfo,
  MemoryInfo,
  MinecraftVersionInfo,
  SavedWorld,
  SearchQuery,
  SearchResult,
  ServerPingStatus,
  ServerResourcePackPolicy,
  Settings,
  SettingsPatch,
} from '@mc/shared'
import { invoke } from './launcher'

export const queryKeys = {
  settings: ['settings'] as const,
  versions: ['versions'] as const,
  accounts: ['accounts'] as const,
  memory: ['memory'] as const,
  java: ['java'] as const,
  instances: ['instances'] as const,
  loaderGames: ['loaderGames'] as const,
  contentSearch: ['content', 'search'] as const,
  contentProject: ['content', 'project'] as const,
  contentVersions: ['content', 'versions'] as const,
  contentPlan: ['content', 'plan'] as const,
  contentWorlds: ['content', 'worlds'] as const,
  integrations: ['integrations'] as const,
}

export function useSettings() {
  return useQuery<Settings>({
    queryKey: queryKeys.settings,
    queryFn: () => invoke('settings:get', undefined),
    staleTime: Number.POSITIVE_INFINITY,
  })
}

export function useMemoryInfo() {
  return useQuery<MemoryInfo>({
    queryKey: queryKeys.memory,
    queryFn: () => invoke('system:memoryInfo', undefined),
    staleTime: 60_000,
  })
}
export function useJavaInstallations() {
  return useQuery<JavaInstallation[]>({
    queryKey: queryKeys.java,
    queryFn: () => invoke('java:detect', undefined),
    staleTime: 30_000,
  })
}
export function useInstances() {
  return useQuery<Instance[]>({
    queryKey: queryKeys.instances,
    queryFn: () => invoke('instances:list', undefined),
    staleTime: 5_000,
  })
}

export function useCreateInstance() {
  const queryClient = useQueryClient()
  return useMutation<Instance, unknown, CreateInstanceRequest>({
    mutationFn: (req) => invoke('instances:create', req),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}

export function useUpdateInstance() {
  const queryClient = useQueryClient()
  return useMutation<void, unknown, Instance>({
    mutationFn: (instance) => invoke('instances:update', instance),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}

export function useRemoveInstance() {
  const queryClient = useQueryClient()
  return useMutation<void, unknown, string>({
    mutationFn: (id) => invoke('instances:remove', { id }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}


export function useValidateJava() {
  return useMutation<JavaInstallation, unknown, string>({
    mutationFn: (path) => invoke('java:validate', { path }),
  })
}


export function useVersions(enabled = true) {
  return useQuery<MinecraftVersionInfo[]>({
    queryKey: queryKeys.versions,
    queryFn: () => invoke('versions:list', undefined),
    enabled,
    staleTime: 5 * 60_000,
    retry: 1,
  })
}

export function useLoaderVersions(
  type: LoaderType,
  minecraftVersion: string,
  enabled = true,
) {
  return useQuery<LoaderVersionInfo[]>({
    queryKey: [...queryKeys.versions, 'loader', type, minecraftVersion],
    queryFn: () => invoke('loaders:versions', { type, minecraftVersion }),
    enabled: enabled && type !== 'vanilla' && minecraftVersion.length > 0,
    staleTime: 10 * 60_000,
    retry: 1,
  })
}

export function useLoaderGames(enabled = true) {
  return useQuery<LoaderGameSupport>({
    queryKey: queryKeys.loaderGames,
    queryFn: () => invoke('loaders:games', undefined),
    enabled,
    staleTime: 10 * 60_000,
    retry: 1,
  })
}

export function useAccounts() {
  return useQuery<Account[]>({
    queryKey: queryKeys.accounts,
    queryFn: () => invoke('accounts:list', undefined),
    staleTime: 5_000,
  })
}

export function useActiveAccount(): { account: Account | null; isLoading: boolean } {
  const settings = useSettings()
  const accounts = useAccounts()
  const activeId = settings.data?.activeAccountId ?? null
  return {
    account: accounts.data?.find((account) => account.id === activeId) ?? null,
    isLoading: settings.isLoading || accounts.isLoading,
  }
}

export function useUpdateSettings() {
  const queryClient = useQueryClient()
  return useMutation<Settings, unknown, SettingsPatch>({
    mutationFn: (patch) => invoke('settings:set', patch),
    onSuccess: (settings) => {
      queryClient.setQueryData(queryKeys.settings, settings)
    },
  })
}

export function useCreateOfflineAccount() {
  const queryClient = useQueryClient()
  return useMutation<Account, unknown, string>({
    mutationFn: (username) => invoke('auth:createOffline', { username }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.accounts }),
        queryClient.invalidateQueries({ queryKey: queryKeys.settings }),
      ])
    },
  })
}

export function useSetActiveAccount() {
  const queryClient = useQueryClient()
  return useMutation<void, unknown, string>({
    mutationFn: (id) => invoke('accounts:setActive', { id }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.settings }),
        queryClient.invalidateQueries({ queryKey: queryKeys.accounts }),
      ])
    },
  })
}

export function useUpdateAccountSkin() {
  const queryClient = useQueryClient()
  return useMutation<
    Account,
    unknown,
    { accountId: string; skinData: string; skinVariant?: 'classic' | 'slim' | undefined; capeData?: string | undefined }
  >({
    mutationFn: (req) => invoke('accounts:updateSkin', req),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.accounts }),
        queryClient.invalidateQueries({ queryKey: queryKeys.settings }),
      ])
    },
  })
}

export function useSyncMinecraftProfile() {
  const queryClient = useQueryClient()
  return useMutation<Account, unknown, { accountId: string }>({
    mutationFn: (req) => invoke('accounts:syncMinecraft', req),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.accounts }),
        queryClient.invalidateQueries({ queryKey: queryKeys.settings }),
      ])
    },
  })
}

export function useSetActiveCape() {
  const queryClient = useQueryClient()
  return useMutation<
    Account,
    unknown,
    { accountId: string; capeId?: string | undefined; customCapeData?: string | undefined }
  >({
    mutationFn: (req) => invoke('accounts:setActiveCape', req),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.accounts }),
        queryClient.invalidateQueries({ queryKey: queryKeys.settings }),
      ])
    },
  })
}

export function useInstallSkinResourcePack() {
  const queryClient = useQueryClient()
  return useMutation<
    { updatedInstances: number; packFileName: string },
    unknown,
    { accountId: string; instanceIds: string[] }
  >({
    mutationFn: (req) => invoke('accounts:installSkinResourcePack', req),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}

export function useBeginMicrosoftLogin() {
  return useMutation<DeviceCodeInfo, unknown, void>({
    mutationFn: () => invoke('auth:beginMicrosoft', undefined),
  })
}

export function useCompleteMicrosoftLogin() {
  const queryClient = useQueryClient()
  return useMutation<Account, unknown, DeviceCodeInfo>({
    mutationFn: (code) => invoke('auth:completeMicrosoft', code),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.accounts }),
        queryClient.invalidateQueries({ queryKey: queryKeys.settings }),
      ])
    },
  })
}

export function useSignOut() {
  const queryClient = useQueryClient()
  return useMutation<void, unknown, string>({
    mutationFn: (accountId) => invoke('auth:signOut', { accountId }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.accounts }),
        queryClient.invalidateQueries({ queryKey: queryKeys.settings }),
      ])
    },
  })
}

/** Kueri pencarian konten; `null` mematikan kueri (mis. tab yang belum didukung). */
export function useContentSearch(query: SearchQuery | null) {
  return useQuery<SearchResult>({
    queryKey: [...queryKeys.contentSearch, query],
    queryFn: () => invoke('content:search', query as SearchQuery),
    enabled: query !== null,
    staleTime: 5 * 60_000,
    retry: 1,
    placeholderData: () => undefined,
  })
}

export function useContentProject(
  provider: ContentProviderId,
  projectId: string | null,
) {
  return useQuery<ContentProject>({
    queryKey: [...queryKeys.contentProject, provider, projectId],
    queryFn: () =>
      invoke('content:project', { provider, projectId: projectId as string }),
    enabled: projectId !== null,
    staleTime: 5 * 60_000,
    retry: 1,
  })
}

export function useContentVersions(
  provider: ContentProviderId,
  projectId: string,
  filter: ContentFilter,
  enabled: boolean,
) {
  return useQuery<ContentVersion[]>({
    queryKey: [...queryKeys.contentVersions, provider, projectId, filter],
    queryFn: () => invoke('content:versions', { provider, projectId, filter }),
    enabled,
    staleTime: 5 * 60_000,
    retry: 1,
  })
}

export function useContentPlan(
  instanceId: string | null,
  provider: ContentProviderId,
  versionId: string | null,
) {
  return useQuery<ContentInstallPlan>({
    queryKey: [...queryKeys.contentPlan, instanceId, provider, versionId],
    queryFn: () =>
      invoke('content:plan', {
        instanceId: instanceId as string,
        provider,
        versionId: versionId as string,
      }),
    enabled: instanceId !== null && versionId !== null,
    staleTime: 5 * 60_000,
    retry: 0,
  })
}

export function useContentWorlds(instanceId: string | null, enabled: boolean) {
  return useQuery<SavedWorld[]>({
    queryKey: [...queryKeys.contentWorlds, instanceId],
    queryFn: () => invoke('content:worlds', { instanceId: instanceId as string }),
    enabled: enabled && instanceId !== null,
    staleTime: 5_000,
  })
}

export function useInstallContent() {
  const queryClient = useQueryClient()
  return useMutation<
    { taskId: string },
    unknown,
    {
      instanceId: string
      provider: ContentProviderId
      versionId: string
      worldId?: string | undefined
    }
  >({
    mutationFn: (req) => invoke('content:install', req),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}

export function useRemoveContent() {
  const queryClient = useQueryClient()
  return useMutation<
    Instance,
    unknown,
    { instanceId: string; provider: ContentProviderId; projectId: string }
  >({
    mutationFn: (req) => invoke('content:remove', req),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}

export function useToggleContent() {
  const queryClient = useQueryClient()
  return useMutation<
    Instance,
    unknown,
    { instanceId: string; provider: ContentProviderId; projectId: string; enabled: boolean }
  >({
    mutationFn: (req) => invoke('content:toggle', req),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}

export function useIntegrations(enabled = true) {
  return useQuery<IntegrationsStatus>({
    queryKey: queryKeys.integrations,
    queryFn: () => invoke('integrations:status', undefined),
    enabled,
    staleTime: 30_000,
  })
}

export function useSetCurseForgeKey() {
  const queryClient = useQueryClient()
  return useMutation<IntegrationsStatus, unknown, string | null>({
    mutationFn: (apiKey) => invoke('integrations:setCurseForgeKey', { apiKey }),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.integrations, status)
    },
  })
}

export function useSetAzureClientId() {
  const queryClient = useQueryClient()
  return useMutation<IntegrationsStatus, unknown, string | null>({
    mutationFn: (clientId) => invoke('integrations:setAzureClientId', { clientId }),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.integrations, status)
    },
  })
}

export function useInstallModpack() {
  const queryClient = useQueryClient()
  return useMutation<{ taskId: string }, unknown, InstallModpackRequest>({
    mutationFn: (req) => invoke('content:installModpack', req),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    },
  })
}

export function useContentUpdates(instanceId: string, enabled = true) {
  return useQuery<ContentUpdateInfo[]>({
    queryKey: ['content', 'updates', instanceId],
    queryFn: () => invoke('content:checkUpdates', { instanceId }),
    enabled: Boolean(instanceId) && enabled,
    staleTime: 60_000,
  })
}

export function useInstanceServers(instanceId: string) {
  return useQuery<InstanceServerEntry[]>({
    queryKey: ['servers', instanceId],
    queryFn: () => invoke('servers:list', { instanceId }),
    enabled: Boolean(instanceId),
  })
}

export function useAddInstanceServer() {
  const queryClient = useQueryClient()
  return useMutation<
    InstanceServerEntry[],
    unknown,
    {
      instanceId: string
      name: string
      ip: string
      resourcePackPolicy?: ServerResourcePackPolicy | undefined
      hideFromHome?: boolean | undefined
    }
  >({
    mutationFn: (req) => invoke('servers:add', req),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({ queryKey: ['servers', vars.instanceId] })
    },
  })
}

export function useRemoveInstanceServer() {
  const queryClient = useQueryClient()
  return useMutation<InstanceServerEntry[], unknown, { instanceId: string; ip: string }>({
    mutationFn: (req) => invoke('servers:remove', req),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({ queryKey: ['servers', vars.instanceId] })
    },
  })
}

export function useEditInstanceServer() {
  const queryClient = useQueryClient()
  return useMutation<
    InstanceServerEntry[],
    unknown,
    {
      instanceId: string
      oldIp: string
      name: string
      newIp: string
      resourcePackPolicy?: ServerResourcePackPolicy | undefined
      hideFromHome?: boolean | undefined
    }
  >({
    mutationFn: (req) => invoke('servers:edit', req),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({ queryKey: ['servers', vars.instanceId] })
    },
  })
}

export function usePingInstanceServer() {
  const queryClient = useQueryClient()
  return useMutation<
    ServerPingStatus,
    unknown,
    { instanceId?: string | undefined; ip: string }
  >({
    mutationFn: (req) => invoke('servers:ping', req),
    onSuccess: (status, vars) => {
      if (status.favicon && vars.instanceId) {
        void queryClient.invalidateQueries({ queryKey: ['servers', vars.instanceId] })
      }
    },
  })
}
