import { invoke } from '@tauri-apps/api/core';

async function ping(value) {
    return await invoke("plugin:tauri-plugin-nostr-sync|ping", {
        payload: {
            value,
        },
    }).then((r) => (r.value ? r.value : null));
}

export { ping };
