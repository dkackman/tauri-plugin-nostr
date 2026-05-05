import { invoke } from '@tauri-apps/api/core';

async function publish(category, payload) {
    return invoke("plugin:nostr-sync|publish", {
        request: { category, payload },
    });
}
async function fetch(category) {
    return invoke("plugin:nostr-sync|fetch", {
        request: { category },
    });
}
async function syncAll(categories) {
    return invoke("plugin:nostr-sync|sync_all", {
        request: { categories },
    });
}
async function addRelay(url) {
    return invoke("plugin:nostr-sync|add_relay", { url });
}
async function removeRelay(url) {
    return invoke("plugin:nostr-sync|remove_relay", { url });
}
async function getRelays() {
    return invoke("plugin:nostr-sync|get_relays");
}
async function getPubkey() {
    return invoke("plugin:nostr-sync|get_pubkey");
}
async function getStatus() {
    return invoke("plugin:nostr-sync|get_status");
}
async function poll(categories) {
    return invoke("plugin:nostr-sync|poll", {
        request: { categories },
    });
}

export { addRelay, fetch, getPubkey, getRelays, getStatus, poll, publish, removeRelay, syncAll };
