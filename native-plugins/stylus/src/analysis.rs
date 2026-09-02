use std::collections::VecDeque;

pub const MAX_GRAPH_SAMPLES: usize = 160;

pub trait TraceSample {
    fn timestamp_us(&self) -> u64;
    fn coordinates(&self) -> (i32, i32);
    fn breaks_stroke(&self) -> bool;
}

#[derive(Default)]
pub struct SamplingAnalysis {
    pub recent_intervals_ms: Vec<f64>,
    pub point_count: usize,
    pub interval_median_ms: f64,
    pub interval_p95_ms: f64,
    pub interval_p99_ms: f64,
    pub interval_max_ms: f64,
    pub interval_stddev_ms: f64,
    pub over_16_7ms: usize,
    pub over_20ms: usize,
    pub over_33_3ms: usize,
    pub turn_median_degrees: f64,
    pub turn_p95_degrees: f64,
}

fn percentile(values: &mut [f64], quantile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let index = ((values.len() - 1) as f64 * quantile).round() as usize;
    values[index.min(values.len() - 1)]
}

pub fn analyze<T: TraceSample>(trace: &VecDeque<T>) -> SamplingAnalysis {
    let Some(stroke_start) = trace.iter().rposition(TraceSample::breaks_stroke) else {
        return SamplingAnalysis::default();
    };
    let stroke = trace.iter().skip(stroke_start).collect::<Vec<_>>();
    if stroke.len() < 2 {
        return SamplingAnalysis {
            point_count: stroke.len(),
            ..Default::default()
        };
    }

    let interval_start = stroke.len().saturating_sub(MAX_GRAPH_SAMPLES + 1);
    let recent = &stroke[interval_start..];
    let mut intervals = Vec::with_capacity(recent.len().saturating_sub(1));
    for pair in recent.windows(2) {
        let elapsed = pair[1]
            .timestamp_us()
            .saturating_sub(pair[0].timestamp_us()) as f64
            / 1000.0;
        if elapsed > 0.0 {
            intervals.push(elapsed);
        }
    }

    let mut turns = Vec::with_capacity(recent.len().saturating_sub(2));
    for points in recent.windows(3) {
        let (x0, y0) = points[0].coordinates();
        let (x1, y1) = points[1].coordinates();
        let (x2, y2) = points[2].coordinates();
        let first = ((y1 - y0) as f64).atan2((x1 - x0) as f64);
        let second = ((y2 - y1) as f64).atan2((x2 - x1) as f64);
        let mut change = (second - first).abs().to_degrees();
        if change > 180.0 {
            change = 360.0 - change;
        }
        if change.is_finite() {
            turns.push(change);
        }
    }

    let mut sorted_intervals = intervals.clone();
    let interval_median_ms = percentile(&mut sorted_intervals, 0.50);
    let interval_p95_ms = percentile(&mut sorted_intervals, 0.95);
    let interval_p99_ms = percentile(&mut sorted_intervals, 0.99);
    let interval_max_ms = intervals.iter().copied().fold(0.0, f64::max);
    let interval_mean_ms = intervals.iter().sum::<f64>() / intervals.len().max(1) as f64;
    let interval_stddev_ms = (intervals
        .iter()
        .map(|interval| (interval - interval_mean_ms).powi(2))
        .sum::<f64>()
        / intervals.len().max(1) as f64)
        .sqrt();
    let over_16_7ms = intervals
        .iter()
        .filter(|interval| **interval > 16.7)
        .count();
    let over_20ms = intervals
        .iter()
        .filter(|interval| **interval > 20.0)
        .count();
    let over_33_3ms = intervals
        .iter()
        .filter(|interval| **interval > 33.3)
        .count();
    let mut sorted_turns = turns;
    let turn_median_degrees = percentile(&mut sorted_turns, 0.50);
    let turn_p95_degrees = percentile(&mut sorted_turns, 0.95);

    SamplingAnalysis {
        recent_intervals_ms: intervals,
        point_count: stroke.len(),
        interval_median_ms,
        interval_p95_ms,
        interval_p99_ms,
        interval_max_ms,
        interval_stddev_ms,
        over_16_7ms,
        over_20ms,
        over_33_3ms,
        turn_median_degrees,
        turn_p95_degrees,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Point(u64, i32, i32, bool);

    impl TraceSample for Point {
        fn timestamp_us(&self) -> u64 {
            self.0
        }
        fn coordinates(&self) -> (i32, i32) {
            (self.1, self.2)
        }
        fn breaks_stroke(&self) -> bool {
            self.3
        }
    }

    #[test]
    fn analysis_uses_only_the_latest_stroke() {
        let trace = VecDeque::from([
            Point(0, 0, 0, true),
            Point(50_000, 1, 0, false),
            Point(100_000, 0, 0, true),
            Point(104_000, 1, 0, false),
            Point(108_000, 2, 0, false),
        ]);
        let result = analyze(&trace);
        assert_eq!(result.point_count, 3);
        assert_eq!(result.recent_intervals_ms, vec![4.0, 4.0]);
        assert_eq!(result.over_20ms, 0);
    }

    #[test]
    fn analysis_counts_long_delivery_gaps() {
        let trace = VecDeque::from([
            Point(0, 0, 0, true),
            Point(4_000, 1, 0, false),
            Point(30_000, 2, 0, false),
        ]);
        let result = analyze(&trace);
        assert_eq!(result.over_16_7ms, 1);
        assert_eq!(result.over_20ms, 1);
        assert_eq!(result.over_33_3ms, 0);
        assert_eq!(result.interval_p99_ms, 26.0);
        assert!(result.interval_stddev_ms > 0.0);
    }

    #[test]
    fn analysis_keeps_multi_second_delivery_gaps() {
        let trace = VecDeque::from([
            Point(0, 0, 0, true),
            Point(4_000, 1, 0, false),
            Point(2_004_000, 2, 0, false),
        ]);
        let result = analyze(&trace);
        assert_eq!(result.recent_intervals_ms, vec![4.0, 2000.0]);
        assert_eq!(result.over_16_7ms, 1);
        assert_eq!(result.over_20ms, 1);
        assert_eq!(result.over_33_3ms, 1);
        assert_eq!(result.interval_max_ms, 2000.0);
    }

    #[test]
    fn analysis_ignores_zero_intervals() {
        let trace = VecDeque::from([
            Point(10_000, 0, 0, true),
            Point(10_000, 1, 0, false),
            Point(10_000, 2, 0, false),
        ]);
        let result = analyze(&trace);
        assert!(result.recent_intervals_ms.is_empty());
        assert_eq!(result.interval_max_ms, 0.0);
    }
}
