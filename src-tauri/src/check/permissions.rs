/// Heuristic mapping of well-known Flutter plugins to the iOS Info.plist
/// usage-description key they need. This is a heuristic, not a permission
/// database: a plugin listed in pubspec.yaml doesn't guarantee the app
/// actually uses the permission at runtime (and the reverse — a plugin not
/// listed here doesn't mean nothing needs a usage description). Add a row
/// whenever a check false-negative comes up for a plugin not covered yet.
pub struct PluginPermission {
    /// Matched as a substring against each pubspec.yaml dependency name.
    pub plugin: &'static str,
    pub info_plist_key: &'static str,
    /// Human label used in the finding message, e.g. "camera access".
    pub label: &'static str,
}

pub const IOS_PLUGIN_PERMISSIONS: &[PluginPermission] = &[
    PluginPermission { plugin: "camera", info_plist_key: "NSCameraUsageDescription", label: "camera access" },
    PluginPermission { plugin: "geolocator", info_plist_key: "NSLocationWhenInUseUsageDescription", label: "location access" },
    PluginPermission { plugin: "location", info_plist_key: "NSLocationWhenInUseUsageDescription", label: "location access" },
    PluginPermission { plugin: "microphone", info_plist_key: "NSMicrophoneUsageDescription", label: "microphone access" },
    PluginPermission { plugin: "speech_to_text", info_plist_key: "NSMicrophoneUsageDescription", label: "microphone access" },
    PluginPermission { plugin: "record", info_plist_key: "NSMicrophoneUsageDescription", label: "microphone access" },
    PluginPermission { plugin: "image_picker", info_plist_key: "NSPhotoLibraryUsageDescription", label: "photo library access" },
    PluginPermission { plugin: "photo_manager", info_plist_key: "NSPhotoLibraryUsageDescription", label: "photo library access" },
    PluginPermission { plugin: "contacts_service", info_plist_key: "NSContactsUsageDescription", label: "contacts access" },
    PluginPermission { plugin: "flutter_contacts", info_plist_key: "NSContactsUsageDescription", label: "contacts access" },
    PluginPermission { plugin: "local_auth", info_plist_key: "NSFaceIDUsageDescription", label: "Face ID access" },
    PluginPermission { plugin: "bluetooth", info_plist_key: "NSBluetoothAlwaysUsageDescription", label: "Bluetooth access" },
];
