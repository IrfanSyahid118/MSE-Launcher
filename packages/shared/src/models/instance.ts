import type { InstanceContentEntry } from './content'

export type LoaderType = 'vanilla' | 'fabric' | 'quilt' | 'neoforge' | 'forge'

export interface LoaderSpec {
  type: LoaderType
  version: string | null
}

export type InstanceJavaMode = 'auto' | 'custom'

export interface InstanceJavaSettings {
  mode: InstanceJavaMode
  path: string | null
}

export interface InstanceMemory {
  overrideGlobal: boolean
  minMb: number | null
  maxMb: number | null
}

export type JvmPreset = 'default' | 'performance' | 'low-memory' | 'custom'

export interface InstanceJvmArgs {
  preset: JvmPreset
  extra: string[]
}

export interface Instance {
  schemaVersion: 1
  id: string
  name: string
  minecraftVersion: string
  loader: LoaderSpec
  java: InstanceJavaSettings
  memory: InstanceMemory
  jvmArgs: InstanceJvmArgs
  content: InstanceContentEntry[]
  createdAt: string
  lastPlayedAt: string | null
  iconUrl?: string | null | undefined
}

export interface CreateInstanceRequest {
  name: string
  minecraftVersion: string
  loader: LoaderSpec
}
