export interface FetchResult {
    category: string;
    payload: unknown;
    updatedAt: string;
    deviceId: string;
}
export interface RelayInfo {
    url: string;
    connected: boolean;
    lastSeen: string | null;
}
export interface SyncStatus {
    ready: boolean;
    relayCount: number;
    connectedRelayCount: number;
    deviceId: string;
}
export declare function publish(category: string, payload: unknown, expiresAt?: number): Promise<void>;
export declare function fetch(category: string): Promise<FetchResult | null>;
export declare function syncAll(categories: string[]): Promise<FetchResult[]>;
export declare function addRelay(url: string): Promise<void>;
export declare function removeRelay(url: string): Promise<void>;
export declare function getRelays(): Promise<RelayInfo[]>;
export declare function getPubkey(): Promise<string | null>;
export declare function getStatus(): Promise<SyncStatus>;
export declare function poll(categories: string[]): Promise<FetchResult[]>;
