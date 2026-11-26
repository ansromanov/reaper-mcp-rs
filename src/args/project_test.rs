use super::*;

#[test]
fn audio_device_action_schema_accepts_only_supported_values() {
    for action in ["status", "close", "open"] {
        let args = serde_json::from_str::<AudioDeviceArgs>(&format!("{{\"action\":\"{action}\"}}"))
            .unwrap();
        assert_eq!(serde_json::to_value(args).unwrap()["action"], action);
    }
    assert!(serde_json::from_str::<AudioDeviceArgs>(r#"{"action":"restart"}"#).is_err());
}
