const COMMANDS: &[&str] = &[
    "publish",
    "fetch",
    "sync_all",
    "add_relay",
    "remove_relay",
    "get_relays",
    "get_pubkey",
    "get_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
