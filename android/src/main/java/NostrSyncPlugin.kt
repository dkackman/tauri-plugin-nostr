package app.tauri.plugin.nostr

import android.app.Activity
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin

@TauriPlugin
class NostrSyncPlugin(private val activity: Activity) : Plugin(activity)
