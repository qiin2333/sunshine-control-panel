//! Format host pipeline rates without substituting the requested FPS for a measurement.

pub(crate) fn format_frame_rates(session: &serde_json::Value) -> String {
    let Some(rates) = session.get("frame_rates") else {
        return "N/A".into();
    };
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

#[cfg(test)]
mod tests {
    use super::format_frame_rates;
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
}
