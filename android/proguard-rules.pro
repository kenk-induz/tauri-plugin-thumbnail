-keepattributes *Annotation*
-keep class app.tauri.plugin.* { *; }
-keep public class * extends app.tauri.plugin.Plugin
-keep public class * extends app.tauri.plugin.Plugin {
    @app.tauri.plugin.Command *;
}
