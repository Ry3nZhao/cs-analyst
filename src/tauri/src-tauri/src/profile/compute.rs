//! Pure profile math: metric definitions, per-match (numerator, denominator) pairs, rolling
//! averages, percentiles and tiers. No database access; see `docs/profile.md` for the rules.

use serde::Serialize;

/// One player's row in one match, as loaded by `load::player_rows`.
#[derive(Debug, Clone, Default)]
pub struct MatchRow {
    pub match_id: String,
    pub steamid: u64,
    pub map_name: Option<String>,
    /// `played_at`, falling back to `discovered_at`.
    pub at: String,
    pub won: Option<bool>,
    pub score_own: u32,
    pub score_opp: u32,
    pub rounds: u32,
    pub kills: u32,
    pub deaths: u32,
    pub headshot_kills: u32,
    pub damage: u32,
    pub kast_rounds: u32,
    pub rating: f64,
    pub swing_total: Option<f64>,
    pub opening_kills: u32,
    pub opening_deaths: u32,
    pub clutch_attempts: u32,
    pub clutch_wins: u32,
    /// Highest N of a won 1vN (0 = none).
    pub best_clutch_won: u32,
    /// Highest multi-kill round (0, 2..=5).
    pub best_multi_kill: u32,
    /// `None` = no aim data for this match (not analysed / failed).
    pub aim: Option<AimRow>,
    /// Platform score snapshot from the demo (`match_players.rank`, `rank_type`).
    pub rank: Option<i64>,
    pub rank_type: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct AimRow {
    pub visibility_reliable: bool,
    pub ttd_n: u32,
    pub ttd_median_ms: Option<f64>,
    pub cp_n: u32,
    pub cp_median_deg: Option<f64>,
    pub cs_shots: u32,
    pub cs_ok: u32,
    pub sprays: u32,
    pub spray_err: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Basic,
    Aim,
}

/// A profile metric. `key` matches the frontend glossary (`metricGlossary.ts`).
#[derive(Debug, Clone, Copy)]
pub struct MetricDef {
    pub key: &'static str,
    pub name: &'static str,
    pub unit: &'static str,
    pub higher_is_better: bool,
    pub category: Category,
    /// Minimum support (rounds, duels, samples, ...) for a sample player to count in percentiles.
    pub min_support: f64,
}

const fn m(key: &'static str, name: &'static str, unit: &'static str, hib: bool, cat: Category, min: f64) -> MetricDef {
    MetricDef { key, name, unit, higher_is_better: hib, category: cat, min_support: min }
}

pub const METRICS: &[MetricDef] = &[
    m("adr", "ADR", "", true, Category::Basic, 10.0),
    m("kast", "KAST", "%", true, Category::Basic, 10.0),
    m("kd", "K/D", "", true, Category::Basic, 10.0),
    m("rating", "HLTV 2.0", "", true, Category::Basic, 10.0),
    m("swing", "Swing", "", true, Category::Basic, 10.0),
    m("openingDuel", "首杀率", "%", true, Category::Basic, 3.0),
    m("clutch", "残局胜率", "%", true, Category::Basic, 2.0),
    m("ttd", "TTD", "ms", false, Category::Aim, 5.0),
    m("crosshairPlacement", "准星位置", "°", false, Category::Aim, 5.0),
    m("counterStrafe", "急停率", "%", true, Category::Aim, 10.0),
    m("sprayError", "压枪误差", "°", false, Category::Aim, 3.0),
];

/// Summable accumulator: value = num / den; `support` gates sample membership.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Acc {
    pub num: f64,
    pub den: f64,
    pub support: f64,
    /// Matches that contributed.
    pub matches: u32,
}

impl Acc {
    pub fn add(&mut self, o: Acc) {
        self.num += o.num;
        self.den += o.den;
        self.support += o.support;
        self.matches += o.matches;
    }
    /// `kd` with zero deaths falls back to kills (as if one death).
    pub fn value(&self) -> Option<f64> {
        if self.matches == 0 {
            None
        } else if self.den > 0.0 {
            Some(self.num / self.den)
        } else if self.support > 0.0 {
            Some(self.num)
        } else {
            None
        }
    }
}

fn acc(num: f64, den: f64, support: f64) -> Option<Acc> {
    Some(Acc { num, den, support, matches: 1 })
}

/// A match's contribution to a metric, or `None` when the metric is unavailable there (e.g. TTD on
/// a demo without reliable visibility). Unavailable is never treated as zero.
pub fn pair(key: &str, r: &MatchRow) -> Option<Acc> {
    let rounds = r.rounds as f64;
    let basic = r.rounds > 0;
    match key {
        "adr" if basic => acc(r.damage as f64, rounds, rounds),
        "kast" if basic => acc(r.kast_rounds as f64 * 100.0, rounds, rounds),
        "kd" if basic => acc(r.kills as f64, r.deaths as f64, rounds),
        "rating" if basic => acc(r.rating * rounds, rounds, rounds),
        "swing" if basic => r.swing_total.and_then(|s| acc(s, rounds, rounds)),
        "openingDuel" => {
            let duels = (r.opening_kills + r.opening_deaths) as f64;
            (duels > 0.0).then(|| acc(r.opening_kills as f64 * 100.0, duels, duels)).flatten()
        }
        "clutch" => {
            let n = r.clutch_attempts as f64;
            (n > 0.0).then(|| acc(r.clutch_wins as f64 * 100.0, n, n)).flatten()
        }
        "ttd" => {
            let a = r.aim.as_ref()?;
            let med = a.ttd_median_ms.filter(|_| a.visibility_reliable && a.ttd_n > 0)?;
            acc(med * a.ttd_n as f64, a.ttd_n as f64, a.ttd_n as f64)
        }
        "crosshairPlacement" => {
            let a = r.aim.as_ref()?;
            let med = a.cp_median_deg.filter(|_| a.visibility_reliable && a.cp_n > 0)?;
            acc(med * a.cp_n as f64, a.cp_n as f64, a.cp_n as f64)
        }
        "counterStrafe" => {
            let a = r.aim.as_ref()?;
            (a.cs_shots > 0).then(|| acc(a.cs_ok as f64 * 100.0, a.cs_shots as f64, a.cs_shots as f64)).flatten()
        }
        "sprayError" => {
            let a = r.aim.as_ref()?;
            let e = a.spray_err.filter(|_| a.sprays > 0)?;
            acc(e * a.sprays as f64, a.sprays as f64, a.sprays as f64)
        }
        _ => None,
    }
}

/// Sum a metric over rows (rows without data are skipped).
pub fn aggregate<'a>(key: &str, rows: impl IntoIterator<Item = &'a MatchRow>) -> Acc {
    let mut a = Acc::default();
    for r in rows {
        if let Some(p) = pair(key, r) {
            a.add(p);
        }
    }
    a
}

/// Rolling value at each position: aggregate of the last `window` matches (current included)
/// that have data. `rows` oldest → newest.
pub fn rolling(key: &str, rows: &[&MatchRow], window: usize) -> Vec<Option<f64>> {
    let window = window.max(1);
    (0..rows.len())
        .map(|i| {
            let lo = (i + 1).saturating_sub(window);
            aggregate(key, rows[lo..=i].iter().copied()).value()
        })
        .collect()
}

/// Percentile (0–100) of `value` within `sample`: share of the sample that is worse, ties count
/// half. Direction-aware (for lower-is-better metrics a lower value ranks higher).
pub fn percentile(value: f64, sample: &[f64], higher_is_better: bool) -> Option<f64> {
    if sample.is_empty() {
        return None;
    }
    let mut worse = 0.0;
    for &s in sample {
        let eq = (s - value).abs() < 1e-9;
        if eq {
            worse += 0.5;
        } else if (s < value) == higher_is_better {
            worse += 1.0;
        }
    }
    Some(100.0 * worse / sample.len() as f64)
}

pub fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 })
}

// ---------------------------------------------------------------------------------------------
// Tiers
// ---------------------------------------------------------------------------------------------

/// Sample needed inside the tier before tier percentiles are used.
pub const MIN_TIER_SAMPLE: usize = 15;
/// Below this (even library-wide) no percentile is shown.
pub const MIN_ANY_SAMPLE: usize = 8;
/// Below this the percentile is flagged as low confidence.
pub const CONFIDENT_SAMPLE: usize = 30;

/// `match_players.rank_type` values (CCSPlayerController.m_iCompetitiveRankType).
pub const RANK_TYPE_WINGMAN: i64 = 7;
pub const RANK_TYPE_PREMIER: i64 = 11;
pub const RANK_TYPE_COMPETITIVE: i64 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreSystem {
    Premier,
    Competitive,
    Wingman,
}

impl ScoreSystem {
    pub fn from_rank_type(t: i64) -> Option<Self> {
        match t {
            RANK_TYPE_PREMIER => Some(Self::Premier),
            RANK_TYPE_COMPETITIVE => Some(Self::Competitive),
            RANK_TYPE_WINGMAN => Some(Self::Wingman),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Premier => "Premier",
            Self::Competitive => "竞技段位",
            Self::Wingman => "搭档段位",
        }
    }
    /// Valid score range (0 = unranked).
    pub fn valid(self, score: i64) -> bool {
        match self {
            Self::Premier => score > 0 && score < 50_000,
            Self::Competitive | Self::Wingman => (1..=18).contains(&score),
        }
    }
}

/// A band of one score system: `[lo, hi]` inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    pub system: ScoreSystem,
    pub lo: i64,
    pub hi: Option<i64>,
    pub label: &'static str,
}

impl Band {
    pub fn contains(&self, system: ScoreSystem, score: i64) -> bool {
        system == self.system && system.valid(score) && score >= self.lo && self.hi.is_none_or(|h| score <= h)
    }
}

const PREMIER_BANDS: &[(i64, Option<i64>, &str)] = &[
    (1, Some(4_999), "灰（< 5000）"),
    (5_000, Some(9_999), "浅蓝（5000–9999）"),
    (10_000, Some(14_999), "蓝（10000–14999）"),
    (15_000, Some(19_999), "紫（15000–19999）"),
    (20_000, Some(24_999), "粉（20000–24999）"),
    (25_000, Some(29_999), "红（25000–29999）"),
    (30_000, None, "金（30000+）"),
];

const SKILL_GROUP_BANDS: &[(i64, Option<i64>, &str)] = &[
    (1, Some(6), "白银（1–6）"),
    (7, Some(10), "黄金新星（7–10）"),
    (11, Some(14), "AK–菊花（11–14）"),
    (15, Some(18), "老鹰及以上（15–18）"),
];

pub fn band_of(system: ScoreSystem, score: i64) -> Option<Band> {
    if !system.valid(score) {
        return None;
    }
    let table = match system {
        ScoreSystem::Premier => PREMIER_BANDS,
        ScoreSystem::Competitive | ScoreSystem::Wingman => SKILL_GROUP_BANDS,
    };
    table
        .iter()
        .find(|(lo, hi, _)| score >= *lo && hi.is_none_or(|h| score <= h))
        .map(|&(lo, hi, label)| Band { system, lo, hi, label })
}

/// The row's platform score, if it is a known, ranked system.
pub fn score_of(r: &MatchRow) -> Option<(ScoreSystem, i64)> {
    let sys = ScoreSystem::from_rank_type(r.rank_type?)?;
    let s = r.rank?;
    sys.valid(s).then_some((sys, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_direction_and_ties() {
        let s = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(3.0, &s, true), Some(62.5)); // 2 worse + half a tie
        assert_eq!(percentile(3.0, &s, false), Some(37.5));
        assert_eq!(percentile(10.0, &s, true), Some(100.0));
        assert_eq!(percentile(0.0, &s, true), Some(0.0));
        assert_eq!(percentile(1.0, &[], true), None);
    }

    #[test]
    fn unavailable_is_not_zero() {
        let mut a = MatchRow { rounds: 10, damage: 800, ..Default::default() };
        let b = MatchRow { rounds: 10, damage: 1200, aim: Some(AimRow { visibility_reliable: true, ttd_n: 4, ttd_median_ms: Some(300.0), ..Default::default() }), ..Default::default() };
        // ESL-like: aim row present but visibility unreliable → TTD unavailable, not 0.
        a.aim = Some(AimRow { visibility_reliable: false, ttd_n: 0, ..Default::default() });
        let rows = [&a, &b];
        assert_eq!(aggregate("ttd", rows).value(), Some(300.0));
        assert_eq!(aggregate("ttd", [&a]).value(), None);
        assert_eq!(aggregate("adr", rows).value(), Some(100.0));
        let r = rolling("ttd", &rows, 2);
        assert_eq!(r, vec![None, Some(300.0)]);
        assert_eq!(rolling("adr", &rows, 1), vec![Some(80.0), Some(120.0)]);
    }

    #[test]
    fn kd_without_deaths() {
        let r = MatchRow { rounds: 13, kills: 7, deaths: 0, ..Default::default() };
        assert_eq!(aggregate("kd", [&r]).value(), Some(7.0));
    }

    #[test]
    fn bands() {
        assert_eq!(band_of(ScoreSystem::Premier, 3392).unwrap().lo, 1);
        assert_eq!(band_of(ScoreSystem::Premier, 15_000).unwrap().hi, Some(19_999));
        assert_eq!(band_of(ScoreSystem::Premier, 31_000).unwrap().hi, None);
        assert!(band_of(ScoreSystem::Premier, 0).is_none());
        assert_eq!(band_of(ScoreSystem::Competitive, 12).unwrap().label, "AK–菊花（11–14）");
        assert!(band_of(ScoreSystem::Competitive, 0).is_none());
    }
}
