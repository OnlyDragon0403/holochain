use gcollections::ops::*;
use interval::{interval_set::*, IntervalSet};
use std::{borrow::Borrow, collections::VecDeque, fmt::Debug, ops::Bound};

use crate::{ArcRange, DhtLocation, PeerView, U32_LEN};

// For u32, IntervalSet excludes MAX from its set of valid values due to its
// need to be able to express the width of an interval using a u32.
// This min and max are set accordingly.
const MIN: u32 = u32::MIN;
const MAX: u32 = u32::MAX - 1;

#[derive(Clone, PartialEq, Eq)]
pub enum DhtArcSet {
    /// Full coverage.
    /// This needs a special representation because the underlying IntervalSet
    /// implementation excludes `u32::MAX` from its set of valid bounds
    Full,
    /// Any coverage other than full, including empty
    Partial(IntervalSet<DhtLocation>),
}

impl std::fmt::Debug for DhtArcSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => f.write_fmt(format_args!("DhtArcSet(Full)",)),
            Self::Partial(intervals) => f.write_fmt(format_args!(
                "DhtArcSet({:#?})",
                intervals.iter().collect::<Vec<_>>()
            )),
        }
    }
}

impl DhtArcSet {
    pub fn new_empty() -> Self {
        Self::Partial(vec![].to_interval_set())
    }

    pub fn new_full() -> Self {
        Self::Full
    }

    pub fn normalized(self) -> Self {
        let make_full = if let Self::Partial(intervals) = &self {
            intervals
                .iter()
                .any(|i| is_full(i.lower().into(), i.upper().into()))
        } else {
            false
        };

        if make_full {
            Self::Full
        } else {
            self
        }
    }

    pub fn from_bounds(start: DhtLocation, end: DhtLocation) -> Self {
        if is_full(start.into(), end.into()) {
            Self::new_full()
        } else {
            Self::Partial(
                if start <= end {
                    vec![(start, end)]
                } else {
                    vec![(MIN.into(), end), (start, MAX.into())]
                }
                .to_interval_set(),
            )
        }
    }

    pub fn from_interval<A: Borrow<ArcInterval>>(arc: A) -> Self {
        match arc.borrow() {
            ArcInterval::Full(_) => Self::new_full(),
            ArcInterval::Empty(_) => Self::new_empty(),
            ArcInterval::Bounded(start, end) => Self::from_bounds(*start, *end),
        }
    }

    pub fn intervals(&self) -> Vec<ArcInterval> {
        match self {
            // XXX: loss of information here, this ArcInterval
            //      does not actually know about its true start
            Self::Full => vec![ArcInterval::Full(0u32.into())],
            Self::Partial(intervals) => {
                let mut intervals: VecDeque<(DhtLocation, DhtLocation)> =
                    intervals.iter().map(|i| (i.lower(), i.upper())).collect();
                let wrapping = match (intervals.front(), intervals.back()) {
                    (Some(first), Some(last)) => {
                        // if there is an interval at the very beginning and one
                        // at the very end, let's interpret it as a single
                        // wrapping interval.
                        //
                        // NB: this checks for values greater than the MAX,
                        // because MAX is not u32::MAX. We don't expect values
                        // greater than MAX, but it's no harm if we do see one.
                        if first.0.as_u32() == MIN && last.1.as_u32() >= MAX {
                            Some((last.0, first.1))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                // Condense the two bookend intervals into single wrapping interval
                if let Some(wrapping) = wrapping {
                    intervals.pop_front();
                    intervals.pop_back();
                    intervals.push_back(wrapping);
                }
                intervals
                    .into_iter()
                    .map(|(lo, hi)| ArcInterval::from_bounds(lo, hi))
                    .collect()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Full => false,
            Self::Partial(intervals) => intervals.is_empty(),
        }
    }

    pub fn contains(&self, t: DhtLocation) -> bool {
        self.overlap(&DhtArcSet::from(vec![(t, t)]))
    }

    /// Cheap check if the two sets have a non-null intersection
    pub fn overlap(&self, other: &Self) -> bool {
        match (self, other) {
            (this, Self::Full) => !this.is_empty(),
            (Self::Full, that) => !that.is_empty(),
            (Self::Partial(this), Self::Partial(that)) => this.overlap(that),
        }
    }

    pub fn union(&self, other: &Self) -> Self {
        match (self, other) {
            (_, Self::Full) => Self::Full,
            (Self::Full, _) => Self::Full,
            (Self::Partial(this), Self::Partial(that)) => {
                Self::Partial(this.union(that)).normalized()
            }
        }
    }

    pub fn intersection(&self, other: &Self) -> Self {
        match (self, other) {
            (this, Self::Full) => this.clone(),
            (Self::Full, that) => that.clone(),
            (Self::Partial(this), Self::Partial(that)) => {
                Self::Partial(this.intersection(that)).normalized()
            }
        }
    }

    pub fn size(&self) -> u32 {
        match self {
            Self::Full => u32::MAX,
            Self::Partial(intervals) => intervals.size(),
        }
    }
}

impl From<&ArcInterval> for DhtArcSet {
    fn from(arc: &ArcInterval) -> Self {
        Self::from_interval(arc)
    }
}

impl From<ArcInterval> for DhtArcSet {
    fn from(arc: ArcInterval) -> Self {
        Self::from_interval(arc)
    }
}

impl From<&[ArcInterval]> for DhtArcSet {
    fn from(arcs: &[ArcInterval]) -> Self {
        arcs.iter()
            .map(Self::from)
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<ArcInterval>> for DhtArcSet {
    fn from(arcs: Vec<ArcInterval>) -> Self {
        arcs.iter()
            .map(Self::from)
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<(DhtLocation, DhtLocation)>> for DhtArcSet {
    fn from(pairs: Vec<(DhtLocation, DhtLocation)>) -> Self {
        pairs
            .into_iter()
            .map(|(a, b)| Self::from(&ArcInterval::from_bounds(a, b)))
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<(u32, u32)>> for DhtArcSet {
    fn from(pairs: Vec<(u32, u32)>) -> Self {
        Self::from(
            pairs
                .into_iter()
                .map(|(a, b)| (DhtLocation::new(a), DhtLocation::new(b)))
                .collect::<Vec<_>>(),
        )
    }
}

#[test]
fn fullness() {
    assert_eq!(DhtArcSet::from(vec![(0, u32::MAX),]), DhtArcSet::Full,);
    assert_eq!(DhtArcSet::from(vec![(0, u32::MAX - 1),]), DhtArcSet::Full,);
    assert_ne!(DhtArcSet::from(vec![(0, u32::MAX - 2),]), DhtArcSet::Full,);

    assert_eq!(DhtArcSet::from(vec![(11, 10),]), DhtArcSet::Full,);

    assert_eq!(
        DhtArcSet::from(vec![(u32::MAX - 1, u32::MAX - 2),]),
        DhtArcSet::Full,
    );

    assert_eq!(
        DhtArcSet::from(vec![(u32::MAX, u32::MAX - 1),]),
        DhtArcSet::Full,
    );
}

/// An alternate implementation of `ArcRange`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArcInterval<T = DhtLocation> {
    Empty(T),
    Full(T),
    Bounded(T, T),
}

impl<T: PartialOrd + num_traits::Num> ArcInterval<T> {
    pub fn contains<B: std::borrow::Borrow<T>>(&self, t: B) -> bool {
        match self {
            Self::Empty(_) => false,
            Self::Full(_) => true,
            Self::Bounded(lo, hi) => {
                let t = t.borrow();
                if lo <= hi {
                    lo <= t && t <= hi
                } else {
                    lo <= t || t <= hi
                }
            }
        }
    }
}

impl<T> ArcInterval<T> {
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> ArcInterval<U> {
        match self {
            Self::Empty(s) => ArcInterval::Empty(f(s)),
            Self::Full(s) => ArcInterval::Full(f(s)),
            Self::Bounded(lo, hi) => ArcInterval::Bounded(f(lo), f(hi)),
        }
    }

    #[deprecated = "left over from refactor"]
    pub fn interval(self) -> Self {
        self
    }
}

impl<T: num_traits::AsPrimitive<u32>> ArcInterval<T> {
    pub fn from_bounds(start: T, end: T) -> ArcInterval<DhtLocation> {
        let start = start.as_();
        let end = end.as_();
        if is_full(start, end) {
            ArcInterval::Full(start.into())
        } else {
            ArcInterval::Bounded(DhtLocation::new(start), DhtLocation::new(end))
        }
    }

    pub fn from_start_and_len(start: T, len: u64) -> ArcInterval<DhtLocation> {
        let start = start.as_();
        if len == 0 {
            ArcInterval::Empty(start.into())
        } else {
            let end = start.wrapping_add((len - 1) as u32);
            ArcInterval::from_bounds(start, end)
        }
    }

    /// Convenience for our legacy code which defined arcs in terms of half-lengths
    /// rather than full lengths
    pub fn from_start_and_halflen(start: T, halflen: u32) -> ArcInterval<DhtLocation> {
        Self::from_start_and_len(start, (halflen as u64) * 2 - 1)
    }

    pub fn new_generic(start: T, end: T) -> Self {
        if is_full(start.as_(), end.as_()) {
            Self::Full(start)
        } else {
            Self::Bounded(start, end)
        }
    }
}

impl ArcInterval<u32> {
    pub fn canonical(self) -> ArcInterval {
        match self {
            ArcInterval::Empty(s) => ArcInterval::Empty(DhtLocation::new(s)),
            ArcInterval::Full(s) => ArcInterval::Full(DhtLocation::new(s)),
            ArcInterval::Bounded(lo, hi) => {
                ArcInterval::from_bounds(DhtLocation::new(lo), DhtLocation::new(hi))
            }
        }
    }
}

impl ArcInterval<DhtLocation> {
    /// Constructor
    pub fn new_empty(s: DhtLocation) -> Self {
        Self::Empty(s)
    }

    /// Represent an arc as an optional range of inclusive endpoints.
    /// If none, the arc length is 0
    pub fn to_bounds_grouped(&self) -> Option<(DhtLocation, DhtLocation)> {
        match self {
            Self::Empty(_) => None,
            Self::Full(s) => Some((*s, s.as_u32().wrapping_sub(1).into())),
            &Self::Bounded(lo, hi) => Some((lo, hi)),
        }
    }

    /// Same as to_bounds_grouped, but with the return type "inside-out"
    pub fn to_primitive_bounds_detached(&self) -> (Option<u32>, Option<u32>) {
        self.to_bounds_grouped()
            .map(|(a, b)| (Some(a.as_u32()), Some(b.as_u32())))
            .unwrap_or_default()
    }

    /// Check if this arc is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty(_))
    }

    /// Check if arcs overlap
    pub fn overlaps(&self, other: &Self) -> bool {
        let a = DhtArcSet::from(self);
        let b = DhtArcSet::from(other);
        a.overlap(&b)
    }

    /// Amount of intersection between two arcs
    pub fn overlap_coverage(&self, other: &Self) -> f64 {
        let a = DhtArcSet::from(self);
        let b = DhtArcSet::from(other);
        let c = a.intersection(&b);
        c.size() as f64 / a.size() as f64
    }

    pub fn start_loc(&self) -> DhtLocation {
        match self {
            ArcInterval::Empty(s) => *s,
            ArcInterval::Full(s) => *s,
            ArcInterval::Bounded(s, _) => *s,
        }
    }

    /// Get the range of the arc
    pub fn range(&self) -> ArcRange {
        match self {
            ArcInterval::Empty(s) => ArcRange {
                start: Bound::Excluded(s.as_u32()),
                end: Bound::Excluded(s.as_u32()),
            },
            ArcInterval::Full(s) => ArcRange {
                start: Bound::Included(s.as_u32()),
                end: Bound::Included(s.as_u32().wrapping_sub(1)),
            },
            ArcInterval::Bounded(lo, hi) => ArcRange {
                start: Bound::Included(lo.as_u32()),
                end: Bound::Included(hi.as_u32()),
            },
        }
    }

    /// The percentage of the full circle that is covered
    /// by this arc.
    pub fn coverage(&self) -> f64 {
        self.length() as f64 / 2f64.powf(32.0)
    }

    pub fn length(&self) -> u64 {
        match self {
            ArcInterval::Empty(_) => 0,
            ArcInterval::Full(_) => 2u64.pow(32),
            ArcInterval::Bounded(lo, hi) => hi.as_u32().wrapping_sub(lo.as_u32()).into(),
        }
    }

    // #[deprecated = "leftover from refactor"]
    pub fn half_length(&self) -> u32 {
        (self.length() / 2) as u32 + 1
    }

    /// Update the half length based on a PeerView reading.
    /// This will converge on a new target instead of jumping directly
    /// to the new target and is designed to be called at a given rate
    /// with more recent peer views.
    pub fn update_length<V: Into<PeerView>>(&mut self, view: V) {
        let new_length = (U32_LEN as f64 * view.into().next_coverage(self.coverage())) as u64;
        *self = Self::from_start_and_len(self.start_loc(), new_length)
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Handy ascii representation of an arc, especially useful when
    /// looking at several arcs at once to get a sense of their overlap
    pub fn to_ascii(&self, len: usize) -> String {
        use crate::{loc_downscale, loc_upscale};

        let empty = || " ".repeat(len);
        let full = || "-".repeat(len);

        // If lo and hi are less than one bucket's width apart when scaled down,
        // decide whether to interpret this as empty or full
        let decide = |lo: &DhtLocation, hi: &DhtLocation| {
            let mid = loc_upscale(len, (len / 2) as i32);
            if lo < hi {
                if hi.as_u32() - lo.as_u32() < mid {
                    empty()
                } else {
                    full()
                }
            } else if lo.as_u32() - hi.as_u32() < mid {
                full()
            } else {
                empty()
            }
        };

        match self {
            Self::Full(_) => full(),
            Self::Empty(_) => empty(),
            Self::Bounded(lo0, hi0) => {
                let lo = loc_downscale(len, *lo0);
                let hi = loc_downscale(len, *hi0);
                let mut s = if lo0 <= hi0 {
                    if lo >= hi {
                        vec![decide(lo0, hi0)]
                    } else {
                        vec![
                            " ".repeat(lo),
                            "-".repeat(hi - lo + 1),
                            " ".repeat((len - hi).saturating_sub(1)),
                        ]
                    }
                } else if lo <= hi {
                    vec![decide(lo0, hi0)]
                } else {
                    vec![
                        "-".repeat(hi + 1),
                        " ".repeat((lo - hi).saturating_sub(1)),
                        "-".repeat(len - lo),
                    ]
                }
                .join("");
                let start = loc_downscale(len, self.start_loc());
                s.replace_range(start..start + 1, "@");
                s
            }
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Ascii representation of an arc, with a histogram of op locations superimposed.
    /// Each character of the string, if an op falls in that "bucket", will be represented
    /// by a hexadecimal digit representing the number of ops in that bucket,
    /// with a max of 0xF (15)
    pub fn to_ascii_with_ops<L: Into<crate::loc8::Loc8>, I: IntoIterator<Item = L>>(
        &self,
        len: usize,
        ops: I,
    ) -> String {
        use crate::{loc8::Loc8, loc_downscale};

        let mut buf = vec![0; len];
        let mut s = self.to_ascii(len);
        for o in ops {
            let o: Loc8 = o.into();
            let o: DhtLocation = o.into();
            let loc = loc_downscale(len, o);
            buf[loc] += 1;
        }
        for (i, v) in buf.into_iter().enumerate() {
            if v > 0 {
                // add hex representation of number of ops in this bucket
                let c = format!("{:x}", v.min(0xf));
                s.replace_range(i..i + 1, &c);
            }
        }
        s
    }

    pub fn canonical(self) -> ArcInterval {
        self
    }
}

/// Check whether a bounded interval is equivalent to the Full interval
fn is_full(start: u32, end: u32) -> bool {
    (start == MIN && end >= MAX) || end == start.wrapping_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_contains() {
        let convergent = ArcInterval::Bounded(10, 20);
        let divergent = ArcInterval::Bounded(20, 10);

        assert!(!convergent.contains(0));
        assert!(!convergent.contains(5));
        assert!(convergent.contains(10));
        assert!(convergent.contains(15));
        assert!(convergent.contains(20));
        assert!(!convergent.contains(25));
        assert!(!convergent.contains(u32::MAX));

        assert!(divergent.contains(0));
        assert!(divergent.contains(5));
        assert!(divergent.contains(10));
        assert!(!divergent.contains(15));
        assert!(divergent.contains(20));
        assert!(divergent.contains(25));
        assert!(divergent.contains(u32::MAX));
    }

    #[test]
    fn test_ascii() {
        let cent = u32::MAX / 100 + 1;
        assert_eq!(
            ArcInterval::from_bounds(cent * 30, cent * 60).to_ascii(10),
            "   -@--   ".to_string()
        );
        assert_eq!(
            ArcInterval::from_bounds(cent * 33, cent * 63).to_ascii(10),
            "   -@--   ".to_string()
        );
        assert_eq!(
            ArcInterval::from_bounds(cent * 29, cent * 59).to_ascii(10),
            "  --@-    ".to_string()
        );

        assert_eq!(
            ArcInterval::from_bounds(cent * 60, cent * 30).to_ascii(10),
            "----  ---@".to_string()
        );
        assert_eq!(
            ArcInterval::from_bounds(cent * 63, cent * 33).to_ascii(10),
            "----  ---@".to_string()
        );
        assert_eq!(
            ArcInterval::from_bounds(cent * 59, cent * 29).to_ascii(10),
            "---  ----@".to_string()
        );

        assert_eq!(
            ArcInterval::from_bounds(cent * 99, cent * 0).to_ascii(10),
            "-        @".to_string()
        );
    }
}
