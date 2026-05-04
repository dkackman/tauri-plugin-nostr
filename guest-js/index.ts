import { invoke } from '@tauri-apps/api/core'

export interface FetchResult {
  category: string
  payload: unknown
  updatedAt: string
  deviceId: string
}

export interface RelayInfo {
  url: string
  connected: boolean
  lastSeen: string | null
}

export interface SyncStatus {
  ready: boolean
  relayCount: number
  connectedRelayCount: number
}

export async function publish(
  category: string,
  payload: unknown
): Promise<void> {
  return invoke('plugin:tauri-plugin-nostr-sync|publish', {
    request: { category, payload },
  })
}

export async function fetch(category: string): Promise<FetchResult | null> {
  return invoke('plugin:tauri-plugin-nostr-sync|fetch', {
    request: { category },
  })
}

export async function syncAll(categories: string[]): Promise<FetchResult[]> {
  return invoke('plugin:tauri-plugin-nostr-sync|sync_all', {
    request: { categories },
  })
}

export async function addRelay(url: string): Promise<void> {
  return invoke('plugin:tauri-plugin-nostr-sync|add_relay', { url })
}

export async function removeRelay(url: string): Promise<void> {
  return invoke('plugin:tauri-plugin-nostr-sync|remove_relay', { url })
}

export async function getRelays(): Promise<RelayInfo[]> {
  return invoke('plugin:tauri-plugin-nostr-sync|get_relays')
}

export async function getPubkey(): Promise<string | null> {
  return invoke('plugin:tauri-plugin-nostr-sync|get_pubkey')
}

export async function getStatus(): Promise<SyncStatus> {
  return invoke('plugin:tauri-plugin-nostr-sync|get_status')
}
