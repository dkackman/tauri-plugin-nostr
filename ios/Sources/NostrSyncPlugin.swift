import SwiftRs
import Tauri
import UIKit
import WebKit

class NostrSyncPlugin: Plugin {}

@_cdecl("init_plugin_tauri_plugin_nostr_sync")
func initPlugin() -> Plugin {
    NostrSyncPlugin()
}
