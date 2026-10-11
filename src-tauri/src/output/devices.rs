use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutputDevice {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// mpv lists every macOS device twice (CoreAudio and AVFoundation). Keep CoreAudio only.
pub(crate) fn parse_device_list(value: &Value) -> Vec<OutputDevice> {
    let mut devices: Vec<OutputDevice> = Vec::new();
    for entry in value.as_array().into_iter().flatten() {
        let Some(id) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        if id == "auto" || id.starts_with("avfoundation/") || devices.iter().any(|d| d.id == id) {
            continue;
        }
        let name = entry
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(id);
        devices.push(OutputDevice {
            id: id.into(),
            name: name.into(),
        });
    }
    devices
}

pub(crate) fn resolve_startup_device(
    saved: Option<&str>,
    devices: &[OutputDevice],
) -> Option<String> {
    saved
        .filter(|id| devices.iter().any(|device| device.id == *id))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_macos_list_without_auto_or_avfoundation_duplicates() {
        let list = json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": "coreaudio/BuiltInSpeakerDevice", "description": "MacBook Pro Speakers"},
            {"name": "coreaudio/3669D23D", "description": "MPG321UX OLED"},
            {"name": "avfoundation/BuiltInSpeakerDevice", "description": "MacBook Pro Speakers"}
        ]);
        assert_eq!(
            parse_device_list(&list),
            vec![
                OutputDevice {
                    id: "coreaudio/BuiltInSpeakerDevice".into(),
                    name: "MacBook Pro Speakers".into()
                },
                OutputDevice {
                    id: "coreaudio/3669D23D".into(),
                    name: "MPG321UX OLED".into()
                },
            ]
        );
    }

    #[test]
    fn parses_linux_list_and_falls_back_to_id_for_blank_names() {
        let list = json!([
            {"name": "pipewire", "description": "Default (pipewire)"},
            {"name": "pulse/alsa_output.usb-DAC", "description": "  "},
            {"name": "pulse/alsa_output.usb-DAC", "description": "Duplicate"}
        ]);
        assert_eq!(
            parse_device_list(&list),
            vec![
                OutputDevice {
                    id: "pipewire".into(),
                    name: "Default (pipewire)".into()
                },
                OutputDevice {
                    id: "pulse/alsa_output.usb-DAC".into(),
                    name: "pulse/alsa_output.usb-DAC".into()
                },
            ]
        );
    }

    #[test]
    fn ignores_malformed_values() {
        assert!(parse_device_list(&json!(true)).is_empty());
        assert!(parse_device_list(&json!([{"description": "No id"}, 4])).is_empty());
    }

    #[test]
    fn resolves_saved_device_only_when_present() {
        let devices = vec![OutputDevice {
            id: "coreaudio/A".into(),
            name: "A".into(),
        }];
        assert_eq!(
            resolve_startup_device(Some("coreaudio/A"), &devices),
            Some("coreaudio/A".into())
        );
        assert_eq!(
            resolve_startup_device(Some("coreaudio/Gone"), &devices),
            None
        );
        assert_eq!(resolve_startup_device(None, &devices), None);
    }
}
