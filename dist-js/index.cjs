'use strict';

var core = require('@tauri-apps/api/core');

async function ping(value) {
    return await core.invoke("plugin:tauri-plugin-nostr|ping", {
        payload: {
            value,
        },
    }).then((r) => (r.value ? r.value : null));
}

exports.ping = ping;
