//! Format host pipeline rates without substituting the requested FPS for a measurement.

pub(crate) fn format_frame_rates(session: &serde_json::Value) -> String {
    let Some(rates) = session.get("frame_rates").filter(|rates| rates.is_object()) else {
        return "N/A".into();
    };
    if rates.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
        return "collection off".into();
    }
    if rates.get("ready").and_then(|v| v.as_bool()) != Some(true) {
        return "warming up".into();
    }
    let source = match rates.get("source_kind").and_then(|v| v.as_str()) {
        Some("vdd_published") => "VDD pub",
        Some("wgc_arrivals") => "WGC arrival",
        Some("dxgi_updates") => "DXGI update",
        _ => "N/A",
    };
    let fps = |stage: &str| {
        rates
            .get(stage)
            .and_then(|v| v.get("fps"))
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "N/A".into())
    };
    format!(
        "Src[{source}] {} | Cap {}\nSubmit {} | Out {} | Repeat {} fps",
        fps("source"),
        fps("capture"),
        fps("submitted"),
        fps("encoded"),
        fps("repeated"),
    )
}

/// Pick one active video session, preferring the latest ID when it is eligible.
pub(crate) fn select_perf_session(body: &serde_json::Value) -> Option<&serde_json::Value> {
    let sessions = body.get("sessions")?.as_array()?;
    let latest_id = body.get("latest_session_id").and_then(|v| v.as_u64());
    let eligible = |session: &&serde_json::Value| {
        session.get("active").and_then(|v| v.as_bool()) == Some(true)
            && session.get("control_only").and_then(|v| v.as_bool()) != Some(true)
            && session.get("session_id").and_then(|v| v.as_u64()).is_some()
    };
    sessions
        .iter()
        .filter(eligible)
        .find(|session| session.get("session_id").and_then(|v| v.as_u64()) == latest_id)
        .or_else(|| {
            sessions
                .iter()
                .filter(eligible)
                .max_by_key(|session| session.get("session_id").and_then(|v| v.as_u64()))
        })
}

/// Match a known perf session ID; without one, preserve standalone runtime metrics.
pub(crate) fn select_runtime_session(
    body: &serde_json::Value,
    session_id: Option<u64>,
) -> Option<&serde_json::Value> {
    let sessions = body.get("sessions")?.as_array()?;
    if let Some(id) = session_id {
        sessions
            .iter()
            .find(|session| session.get("session_id").and_then(|v| v.as_u64()) == Some(id))
    } else {
        sessions.first()
    }
}

#[cfg(test)]
mod tests {
    use super::{format_frame_rates, select_perf_session, select_runtime_session};
    use serde_json::json;

    #[test]
    fn exposes_source_capture_gap_and_repeated_submissions() {
        let session = json!({
            "fps": 120,
            "frame_rates": {
                "ready": true, "source_kind": "vdd_published",
                "source": {"fps": 120.0}, "capture": {"fps": 110.0},
                "submitted": {"fps": 120.0}, "encoded": {"fps": 119.5},
                "repeated": {"fps": 10.0}
            }
        });
        assert_eq!(
            format_frame_rates(&session),
            "Src[VDD pub] 120.0 | Cap 110.0\nSubmit 120.0 | Out 119.5 | Repeat 10.0 fps"
        );
    }

    #[test]
    fn missing_measurements_never_use_the_target_fps() {
        assert_eq!(format_frame_rates(&json!({"fps": 120})), "N/A");
        assert_eq!(
            format_frame_rates(&json!({"frame_rates": {
                "enabled": false, "ready": true, "capture": {"fps": 120}
            }})),
            "collection off"
        );
        for value in [
            json!(null),
            json!("invalid"),
            json!(120),
            json!(true),
            json!([]),
        ] {
            assert_eq!(format_frame_rates(&json!({"frame_rates": value})), "N/A");
        }
        assert_eq!(
            format_frame_rates(&json!({"frame_rates": {"ready": false}})),
            "warming up"
        );
        let text = format_frame_rates(&json!({"fps": 120, "frame_rates": {
            "ready": true, "source_kind": "unavailable", "source": {"fps": null},
            "capture": {"fps": 0.0}, "submitted": {"fps": 0.0},
            "encoded": {"fps": 0.0}, "repeated": {"fps": 0.0}
        }}));
        assert_eq!(
            text,
            "Src[N/A] N/A | Cap 0.0\nSubmit 0.0 | Out 0.0 | Repeat 0.0 fps"
        );
    }

    #[test]
    fn selects_the_same_video_session_across_differently_ordered_snapshots() {
        let perf = json!({"latest_session_id": 2, "sessions": [
            {"session_id": 2, "active": true, "fps": 120},
            {"session_id": 1, "active": true, "fps": 60}
        ]});
        let runtime = json!({"sessions": [
            {"session_id": 1, "fps": 60}, {"session_id": 2, "fps": 120}
        ]});
        let selected = select_perf_session(&perf).unwrap();
        let id = selected["session_id"].as_u64();
        let matched = select_runtime_session(&runtime, id).unwrap();
        assert_eq!(selected["session_id"], matched["session_id"]);
        assert_eq!(matched["fps"], 120);
        let mut older_latest = perf.clone();
        older_latest["latest_session_id"] = json!(1);
        assert_eq!(select_perf_session(&older_latest).unwrap()["session_id"], 1);
    }

    #[test]
    fn skips_control_only_and_ended_sessions_and_uses_a_stable_fallback() {
        let perf = json!({"latest_session_id": 4, "sessions": [
            {"session_id": 2, "active": true},
            {"session_id": 4, "active": true, "control_only": true},
            {"session_id": 5, "active": false},
            {"session_id": 3, "active": true},
            {"active": true}
        ]});
        assert_eq!(select_perf_session(&perf).unwrap()["session_id"], 3);
        let mut reordered = perf.clone();
        reordered["sessions"].as_array_mut().unwrap().reverse();
        assert_eq!(select_perf_session(&reordered).unwrap()["session_id"], 3);
        assert!(select_perf_session(&json!({"sessions": [
            {"session_id": 4, "active": true, "control_only": true}
        ]}))
        .is_none());
    }

    #[test]
    fn absent_runtime_match_never_falls_back_to_another_session() {
        let runtime = json!({"sessions": [{"session_id": 1, "fps": 60}]});
        assert!(select_runtime_session(&runtime, Some(2)).is_none());
    }

    #[test]
    fn unavailable_perf_preserves_runtime_metrics_and_recovery_restores_id_matching() {
        let runtime = json!({"sessions": [
            {"session_id": 1, "state": "RUNNING", "fps": 60},
            {"session_id": 2, "state": "RUNNING", "fps": 120}
        ]});
        // Network failures, older hosts and snapshots with no video session yield no ID.
        for perf in [
            json!(null),
            json!({"success": false}),
            json!({"sessions": []}),
        ] {
            let id = select_perf_session(&perf).and_then(|s| s["session_id"].as_u64());
            let session = select_runtime_session(&runtime, id).unwrap();
            assert_eq!(session["state"], "RUNNING");
            assert_eq!(session["fps"], 60);
        }
        let recovered = json!({"latest_session_id": 2, "sessions": [
            {"session_id": 2, "active": true}
        ]});
        let id = select_perf_session(&recovered).and_then(|s| s["session_id"].as_u64());
        assert_eq!(select_runtime_session(&runtime, id).unwrap()["fps"], 120);
        assert!(select_runtime_session(&json!({"sessions": []}), None).is_none());
    }
}
