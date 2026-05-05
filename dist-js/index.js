import { invoke } from '@tauri-apps/api/core';

async function publish(category, payload) {
    return invoke("plugin:tauri-plugin-nostr-sync|publish", {
        request: { category, payload },
    });
}
async function fetch(category) {
    return invoke("plugin:tauri-plugin-nostr-sync|fetch", {
        request: { category },
    });
}
async function syncAll(categories) {
    return invoke("plugin:tauri-plugin-nostr-sync|sync_all", {
        request: { categories },
    });
}
async function addRelay(url) {
    return invoke("plugin:tauri-plugin-nostr-sync|add_relay", { url });
}
async function removeRelay(url) {
    return invoke("plugin:tauri-plugin-nostr-sync|remove_relay", { url });
}
async function getRelays() {
    return invoke("plugin:tauri-plugin-nostr-sync|get_relays");
}
async function getPubkey() {
    return invoke("plugin:tauri-plugin-nostr-sync|get_pubkey");
}
async function getStatus() {
    return invoke("plugin:tauri-plugin-nostr-sync|get_status");
}

export { addRelay, fetch, getPubkey, getRelays, getStatus, publish, removeRelay, syncAll };
