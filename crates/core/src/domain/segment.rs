//! Time segments: one normalized model shared by keep, remove and export.

use std::fmt;

use super::time::TimeMs;

/// A half-open time range `[start, end)` in a media file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Segment {
    start: TimeMs,
    end: TimeMs,
}

impl Segment {
    /// Builds a segment without checking it; `normalize` checks it.
    pub fn new(start: TimeMs, end: TimeMs) -> Segment {
        Segment { start, end }
    }

    /// The first millisecond of the segment.
    pub fn start(&self) -> TimeMs {
        self.start
    }

    /// The millisecond where the segment stops.
    pub fn end(&self) -> TimeMs {
        self.end
    }

    /// The length in milliseconds; zero when the end is not after the start.
    pub fn len_ms(&self) -> u64 {
        self.end.0.saturating_sub(self.start.0)
    }
}

/// Why a list of segments cannot be normalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentError {
    /// There are no segments with a length above zero.
    Empty,
    /// A segment starts after it ends.
    StartAfterEnd,
    /// A segment starts at or after the end of the source.
    BeyondDuration,
}

impl fmt::Display for SegmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SegmentError::Empty => "no segment has a length above zero",
            SegmentError::StartAfterEnd => "a segment starts after it ends",
            SegmentError::BeyondDuration => "a segment starts at or after the end of the source",
        };
        f.write_str(text)
    }
}

impl std::error::Error for SegmentError {}

/// Sorts, clamps and merges segments against a source of `duration`.
///
/// Zero-length segments are dropped. A segment that ends after `duration` is
/// clamped to it. Overlapping and touching segments are merged. The result is
/// sorted and disjoint, and each segment has a start below its end.
pub fn normalize(segments: &[Segment], duration: TimeMs) -> Result<Vec<Segment>, SegmentError> {
    if segments.is_empty() {
        return Err(SegmentError::Empty);
    }
    if segments.iter().any(|s| s.start > s.end) {
        return Err(SegmentError::StartAfterEnd);
    }
    let mut kept: Vec<Segment> = segments
        .iter()
        .copied()
        .filter(|s| s.start != s.end)
        .collect();
    if kept.is_empty() {
        return Err(SegmentError::Empty);
    }
    if kept.iter().any(|s| s.start >= duration) {
        return Err(SegmentError::BeyondDuration);
    }
    for s in &mut kept {
        if s.end > duration {
            s.end = duration;
        }
    }
    kept.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    let mut merged: Vec<Segment> = Vec::with_capacity(kept.len());
    for s in kept {
        match merged.last_mut() {
            Some(last) if s.start <= last.end => {
                if s.end > last.end {
                    last.end = s.end;
                }
            }
            _ => merged.push(s),
        }
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn seg(start: u64, end: u64) -> Segment {
        Segment::new(TimeMs(start), TimeMs(end))
    }

    #[test]
    fn reversed_segment_is_an_error() {
        assert_eq!(
            normalize(&[seg(30, 10)], TimeMs(1_000)),
            Err(SegmentError::StartAfterEnd)
        );
    }

    #[test]
    fn zero_length_segment_is_dropped() {
        assert_eq!(
            normalize(&[seg(100, 100), seg(200, 300)], TimeMs(1_000)),
            Ok(vec![seg(200, 300)])
        );
    }

    #[test]
    fn segment_past_end_is_clamped() {
        assert_eq!(
            normalize(&[seg(500, 2_000)], TimeMs(1_000)),
            Ok(vec![seg(500, 1_000)])
        );
    }

    #[test]
    fn segment_entirely_past_end_is_an_error() {
        assert_eq!(
            normalize(&[seg(1_500, 2_000)], TimeMs(1_000)),
            Err(SegmentError::BeyondDuration)
        );
        assert_eq!(
            normalize(&[seg(1_000, 1_200)], TimeMs(1_000)),
            Err(SegmentError::BeyondDuration)
        );
    }

    #[test]
    fn duration_zero_with_a_real_segment_is_beyond_duration() {
        assert_eq!(
            normalize(&[seg(0, 5)], TimeMs(0)),
            Err(SegmentError::BeyondDuration)
        );
    }

    #[test]
    fn overlapping_segments_merge() {
        assert_eq!(
            normalize(&[seg(0, 15), seg(10, 20)], TimeMs(1_000)),
            Ok(vec![seg(0, 20)])
        );
    }

    #[test]
    fn touching_segments_merge() {
        assert_eq!(
            normalize(&[seg(0, 10), seg(10, 20)], TimeMs(1_000)),
            Ok(vec![seg(0, 20)])
        );
    }

    #[test]
    fn three_way_merge_chain() {
        assert_eq!(
            normalize(&[seg(0, 10), seg(20, 30), seg(9, 21)], TimeMs(1_000)),
            Ok(vec![seg(0, 30)])
        );
    }

    #[test]
    fn unsorted_input_is_sorted() {
        assert_eq!(
            normalize(&[seg(50, 60), seg(0, 10)], TimeMs(1_000)),
            Ok(vec![seg(0, 10), seg(50, 60)])
        );
    }

    #[test]
    fn all_empty_input_is_an_error() {
        assert_eq!(normalize(&[], TimeMs(1_000)), Err(SegmentError::Empty));
        assert_eq!(
            normalize(&[seg(5, 5), seg(7, 7)], TimeMs(1_000)),
            Err(SegmentError::Empty)
        );
    }

    #[test]
    fn contained_segment_is_absorbed() {
        assert_eq!(
            normalize(&[seg(0, 100), seg(10, 20)], TimeMs(1_000)),
            Ok(vec![seg(0, 100)])
        );
        assert_eq!(
            normalize(&[seg(10, 20), seg(0, 100)], TimeMs(1_000)),
            Ok(vec![seg(0, 100)])
        );
    }

    #[test]
    fn any_segment_beyond_the_duration_fails_the_whole_call() {
        assert_eq!(
            normalize(&[seg(0, 10), seg(1_500, 2_000)], TimeMs(1_000)),
            Err(SegmentError::BeyondDuration)
        );
    }

    #[test]
    fn zero_length_segment_beyond_the_duration_is_dropped_not_an_error() {
        assert_eq!(
            normalize(&[seg(2_000, 2_000), seg(100, 200)], TimeMs(1_000)),
            Ok(vec![seg(100, 200)])
        );
    }

    #[test]
    fn reversed_segment_is_reported_before_the_duration_check() {
        assert_eq!(
            normalize(&[seg(2_000, 1_500)], TimeMs(1_000)),
            Err(SegmentError::StartAfterEnd)
        );
    }

    #[test]
    fn reversed_segment_among_valid_ones_is_reported() {
        assert_eq!(
            normalize(&[seg(0, 10), seg(30, 20)], TimeMs(1_000)),
            Err(SegmentError::StartAfterEnd)
        );
        assert_eq!(
            normalize(&[seg(30, 20), seg(0, 10)], TimeMs(1_000)),
            Err(SegmentError::StartAfterEnd)
        );
    }

    #[test]
    fn end_exactly_at_the_duration_is_unchanged() {
        assert_eq!(
            normalize(&[seg(500, 1_000)], TimeMs(1_000)),
            Ok(vec![seg(500, 1_000)])
        );
    }

    #[test]
    fn identical_duplicates_collapse_to_one() {
        assert_eq!(
            normalize(&[seg(5, 10), seg(5, 10)], TimeMs(1_000)),
            Ok(vec![seg(5, 10)])
        );
    }

    #[test]
    fn single_segment_is_returned_unchanged() {
        assert_eq!(
            normalize(&[seg(5, 10)], TimeMs(1_000)),
            Ok(vec![seg(5, 10)])
        );
    }

    #[test]
    fn len_ms_saturates_at_zero_for_reversed_segment() {
        assert_eq!(seg(10, 30).len_ms(), 20);
        assert_eq!(seg(30, 10).len_ms(), 0);
    }

    fn segment_strategy() -> impl Strategy<Value = Segment> {
        (0u64..=1_000, 0u64..=1_000).prop_map(|(a, b)| {
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            seg(start, end)
        })
    }

    proptest! {
        #[test]
        fn output_is_sorted_disjoint_and_within_duration(
            segments in prop::collection::vec(segment_strategy(), 0..=12),
            duration in prop_oneof![0u64..=1_200, 0u64..=1_000_000],
        ) {
            if let Ok(out) = normalize(&segments, TimeMs(duration)) {
                for s in &out {
                    prop_assert!(s.start() < s.end());
                    prop_assert!(s.end().0 <= duration);
                }
                for pair in out.windows(2) {
                    prop_assert!(pair[0].start() <= pair[1].start());
                    prop_assert!(pair[1].start() > pair[0].end());
                }
            }
        }

        #[test]
        fn normalize_is_idempotent(
            segments in prop::collection::vec(segment_strategy(), 0..=12),
            duration in prop_oneof![0u64..=1_200, 0u64..=1_000_000],
        ) {
            if let Ok(once) = normalize(&segments, TimeMs(duration)) {
                let twice = normalize(&once, TimeMs(duration));
                prop_assert_eq!(twice, Ok(once));
            }
        }

        #[test]
        fn output_covers_exactly_the_union_of_the_clamped_inputs(
            segments in prop::collection::vec(segment_strategy(), 0..=12),
            duration in 0u64..=1_200,
        ) {
            prop_assume!(segments.iter().any(|s| s.len_ms() > 0));
            prop_assume!(segments.iter().all(|s| s.len_ms() == 0 || s.start().0 < duration));
            let out = normalize(&segments, TimeMs(duration))
                .map_err(|e| TestCaseError::fail(e.to_string()))?;

            let dur = duration as usize;
            let mut expected = vec![false; dur];
            for s in &segments {
                let from = s.start().0.min(duration) as usize;
                let to = s.end().0.min(duration) as usize;
                for slot in &mut expected[from..to] {
                    *slot = true;
                }
            }
            let mut actual = vec![false; dur];
            for s in &out {
                for slot in &mut actual[s.start().0 as usize..s.end().0 as usize] {
                    *slot = true;
                }
            }
            prop_assert_eq!(&actual, &expected);

            let runs = expected
                .iter()
                .enumerate()
                .filter(|&(i, &v)| v && (i == 0 || !expected[i - 1]))
                .count();
            prop_assert_eq!(out.len(), runs);
        }
    }
}
