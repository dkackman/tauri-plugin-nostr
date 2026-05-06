'use strict';

var core = require('@tauri-apps/api/core');

async function publish(category, payload, expiresAt) {
    return core.invoke("plugin:nostr-sync|publish", {
        request: { category, payload, expiration: expiresAt ?? null },
    });
}
async function fetch(category) {
    return core.invoke("plugin:nostr-sync|fetch", {
        request: { category },
    });
}
async function syncAll(categories) {
    return core.invoke("plugin:nostr-sync|sync_all", {
        request: { categories },
    });
}
async function addRelay(url) {
    return core.invoke("plugin:nostr-sync|add_relay", { url });
}
async function removeRelay(url) {
    return core.invoke("plugin:nostr-sync|remove_relay", { url });
}
async function getRelays() {
    return core.invoke("plugin:nostr-sync|get_relays");
}
async function getPubkey() {
    return core.invoke("plugin:nostr-sync|get_pubkey");
}
async function getStatus() {
    return core.invoke("plugin:nostr-sync|get_status");
}
async function poll(categories) {
    return core.invoke("plugin:nostr-sync|poll", {
        request: { categories },
    });
}

exports.addRelay = addRelay;
exports.fetch = fetch;
exports.getPubkey = getPubkey;
exports.getRelays = getRelays;
exports.getStatus = getStatus;
exports.poll = poll;
exports.publish = publish;
exports.removeRelay = removeRelay;
exports.syncAll = syncAll;
