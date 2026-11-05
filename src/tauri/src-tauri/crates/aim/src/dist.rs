//! Small distribution summaries.

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dist {
    pub n: u32,
    pub mean: Option<f32>,
    pub median: Option<f32>,
    pub p25: Option<f32>,
    pub p75: Option<f32>,
}

/// Linear-interpolated quantile of sorted data.
fn quantile(sorted: &[f32], q: f32) -> f32 {
    let pos = q * (sorted.len() - 1) as f32;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f32)
}

impl Dist {
    pub fn of(values: impl IntoIterator<Item = f32>) -> Self {
        let mut v: Vec<f32> = values.into_iter().filter(|x| x.is_finite()).collect();
        if v.is_empty() {
            return Self::default();
        }
        v.sort_by(|a, b| a.total_cmp(b));
        Self {
            n: v.len() as u32,
            mean: Some(v.iter().sum::<f32>() / v.len() as f32),
            median: Some(quantile(&v, 0.5)),
            p25: Some(quantile(&v, 0.25)),
            p75: Some(quantile(&v, 0.75)),
        }
    }
}

/// Counts in `bins` equal-width bins over `[0, bins * width)`; the last bin also takes overflow.
pub fn histogram(values: impl IntoIterator<Item = f32>, width: f32, bins: usize) -> Vec<u32> {
    let mut h = vec![0u32; bins];
    for x in values {
        if x.is_finite() && x >= 0.0 {
            h[((x / width) as usize).min(bins - 1)] += 1;
        }
    }
    h
}

pub fn ratio(num: u32, den: u32) -> Option<f32> {
    (den > 0).then(|| num as f32 / den as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dist_basics() {
        let d = Dist::of([4.0, 1.0, 3.0, 2.0]);
        assert_eq!(d.n, 4);
        assert_eq!(d.median, Some(2.5));
        assert_eq!(d.mean, Some(2.5));
        assert_eq!(Dist::of([]).median, None);
        assert_eq!(histogram([0.0, 0.9, 1.0, 99.0], 1.0, 3), vec![2, 1, 1]);
    }
}
