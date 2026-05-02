import SwiftRs
import Tauri
import UIKit
import WebKit

class PingArgs: Decodable {
    let value: String?
}

class ExamplePlugin: Plugin {
    @objc func ping(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(PingArgs.self)
        invoke.resolve(["value": args.value ?? ""])
    }
}

@_cdecl("init_plugin_tauri_plugin_nostr")
func initPlugin() -> Plugin {
    ExamplePlugin()
}
