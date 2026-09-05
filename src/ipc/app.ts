import { invoke } from './invoke'

export interface AppInfo {
  appVersion: string
  schemaVersion: number
  latestSchemaVersion: number
  dbPath: string
  setupCompleted: boolean
}

export const appApi = {
  info: () => invoke<AppInfo>('app_info'),
}
