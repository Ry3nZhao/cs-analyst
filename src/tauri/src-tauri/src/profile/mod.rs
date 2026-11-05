//! Personal profile (W3-B, docs/roadmap.md 2.10, docs/profile.md): trends, map / weapon / side
//! breakdowns, personal bests, local same-tier percentiles and focus areas. Computed on demand
//! from stored stats / aim rows; there is no cache table (migration 011 is not used).

pub mod commands;
pub mod compute;
pub mod load;

#[cfg(test)]
mod tests;

use crate::database::{Database, Result};
use compute::{Acc, Band, MatchRow, ScoreSystem, METRICS};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const DEFAULT_WINDOW: u32 = 10;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileOptions {
    /// Trend rolling window (matches). Default 10.
    pub window: Option<u32>,
    /// Restrict the profile to the newest N matches; `None` = all.
    pub last_matches: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub steamid: String,
    /// Every steam identity of the person (W1-C merges).
    pub steamids: Vec<String>,
    pub person_id: Option<i64>,
    pub display_name: Option<String>,
    pub window: u32,
    pub last_matches: Option<u32>,
    pub summary: Summary,
    pub tier: Option<Tier>,
    pub scores: Vec<ScoreSeries>,
    pub sample: SampleInfo,
    pub metrics: Vec<MetricSummary>,
    pub trend: Trend,
    pub maps: Vec<MapRow>,
    pub sides: Vec<SideRow>,
    pub weapons: Vec<WeaponRow>,
    pub bests: Vec<PersonalBest>,
    pub focus: Vec<FocusItem>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub matches: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    /// wins / (wins + losses + draws), 0–100.
    pub win_rate: Option<f64>,
    pub rounds: u32,
    pub kills: u32,
    pub deaths: u32,
    pub first_match_at: Option<String>,
    pub last_match_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tier {
    pub system: ScoreSystem,
    pub system_label: &'static str,
    pub score: i64,
    pub band_lo: i64,
    pub band_hi: Option<i64>,
    pub band_label: &'static str,
    pub match_id: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreSeries {
    pub system: ScoreSystem,
    pub label: &'static str,
    /// Oldest → newest.
    pub points: Vec<ScorePoint>,
    pub latest: i64,
    pub best: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScorePoint {
    pub match_id: String,
    pub at: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleInfo {
    /// Other players (≥ 10 rounds) inside the tier; 0 without a tier.
    pub tier_players: u32,
    /// Other players (≥ 10 rounds) in the whole library.
    pub library_players: u32,
    pub min_tier_sample: u32,
    pub min_sample: u32,
    pub confident_sample: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SampleScope {
    /// Same-tier players.
    Tier,
    /// Not enough same-tier players: the whole local library.
    All,
    /// Not enough players at all: no percentile.
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Percentile {
    /// 0–100, higher = better than more of the sample. `None` when the sample is too small or
    /// the player has no value.
    pub value: Option<f64>,
    pub scope: SampleScope,
    /// Players in the sample actually used (or in the larger pool when `scope = none`).
    pub sample_size: u32,
    pub sample_median: Option<f64>,
    /// Small sample or little support for the player's own value.
    pub low_confidence: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSummary {
    pub key: &'static str,
    pub name: &'static str,
    pub unit: &'static str,
    pub higher_is_better: bool,
    pub category: compute::Category,
    /// Over the profile scope; `None` = unavailable (never 0 by default).
    pub value: Option<f64>,
    /// Over the newest `window` matches.
    pub recent: Option<f64>,
    pub matches_with_data: u32,
    pub support: f64,
    pub percentile: Percentile,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trend {
    pub window: u32,
    /// Oldest → newest.
    pub matches: Vec<TrendMatch>,
    pub series: Vec<TrendSeries>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendMatch {
    pub match_id: String,
    pub at: String,
    pub map_name: Option<String>,
    /// win | loss | draw
    pub result: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendSeries {
    pub key: &'static str,
    /// Per match (`None` = unavailable in that match).
    pub values: Vec<Option<f64>>,
    /// Rolling aggregate over the last `window` matches with data.
    pub rolling: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapRow {
    pub map_name: String,
    pub matches: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub win_rate: Option<f64>,
    pub rounds: u32,
    pub adr: Option<f64>,
    pub kast: Option<f64>,
    pub kd: Option<f64>,
    pub rating: Option<f64>,
    pub swing: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SideRow {
    /// 2 = T, 3 = CT.
    pub side: u8,
    pub rounds: u32,
    pub rounds_won: u32,
    pub round_win_rate: Option<f64>,
    pub kills: u32,
    pub deaths: u32,
    pub kd: Option<f64>,
    pub adr: Option<f64>,
    pub kast: Option<f64>,
    pub opening_kills: u32,
    pub opening_deaths: u32,
    pub opening_rate: Option<f64>,
    pub swing: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponRow {
    pub weapon: String,
    pub kills: u32,
    pub headshot_kills: u32,
    pub hs_pct: Option<f64>,
    /// Aim data (guns only; 0 when not analysed).
    pub shots: u32,
    pub hits: u32,
    pub accuracy: Option<f64>,
    pub head_hit_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalBest {
    /// kills | adr | rating | swing | headshotKills | multiKill | clutch | ttd
    pub key: &'static str,
    pub label: &'static str,
    /// Metric glossary key for `MetricHelp`.
    pub metric: &'static str,
    pub value: f64,
    pub match_id: String,
    pub map_name: Option<String>,
    pub at: String,
    pub result: &'static str,
    pub score_own: u32,
    pub score_opp: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusItem {
    pub key: &'static str,
    pub name: &'static str,
    pub value: f64,
    pub percentile: f64,
    pub sample_median: Option<f64>,
    pub scope: SampleScope,
    pub sample_size: u32,
}

fn rate(num: f64, den: f64) -> Option<f64> {
    (den > 0.0).then(|| num / den)
}

fn result_of(r: &MatchRow) -> &'static str {
    match r.won {
        Some(true) => "win",
        Some(false) => "loss",
        None => "draw",
    }
}

/// Minimum rounds for a player to count in `SampleInfo` player counts.
const MIN_PLAYER_ROUNDS: u32 = 10;
/// Max focus areas.
const MAX_FOCUS: usize = 3;
/// Only metrics below this percentile are focus candidates.
const FOCUS_BELOW: f64 = 50.0;

/// Per-steamid aggregate of `key` over `rows`, keeping players with enough support.
fn sample_values(key: &str, min_support: f64, rows: &[&MatchRow]) -> Vec<f64> {
    let mut by: HashMap<u64, Acc> = HashMap::new();
    for r in rows {
        if let Some(p) = compute::pair(key, r) {
            by.entry(r.steamid).or_default().add(p);
        }
    }
    let mut v: Vec<f64> = by.values().filter(|a| a.support >= min_support).filter_map(|a| a.value()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v
}

fn count_players(rows: &[&MatchRow]) -> u32 {
    let mut rounds: HashMap<u64, u32> = HashMap::new();
    for r in rows {
        *rounds.entry(r.steamid).or_default() += r.rounds;
    }
    rounds.values().filter(|&&n| n >= MIN_PLAYER_ROUNDS).count() as u32
}

fn percentile_for(def: &compute::MetricDef, mine: &Acc, tier_pool: Option<&[&MatchRow]>, lib_pool: &[&MatchRow]) -> Percentile {
    let lib = sample_values(def.key, def.min_support, lib_pool);
    let tier = tier_pool.map(|p| sample_values(def.key, def.min_support, p)).unwrap_or_default();
    let (scope, sample) = if tier.len() >= compute::MIN_TIER_SAMPLE {
        (SampleScope::Tier, tier)
    } else if lib.len() >= compute::MIN_ANY_SAMPLE {
        (SampleScope::All, lib)
    } else {
        (SampleScope::None, lib)
    };
    let value = match (scope, mine.value()) {
        (SampleScope::None, _) | (_, None) => None,
        (_, Some(v)) => compute::percentile(v, &sample, def.higher_is_better),
    };
    Percentile {
        value,
        scope,
        sample_size: sample.len() as u32,
        sample_median: compute::median(&sample),
        low_confidence: sample.len() < compute::CONFIDENT_SAMPLE || mine.support < def.min_support,
    }
}

fn find_tier(mine: &[MatchRow]) -> Option<(Band, Tier)> {
    let r = mine.iter().rev().find(|r| compute::score_of(r).is_some())?;
    let (system, score) = compute::score_of(r)?;
    let band = compute::band_of(system, score)?;
    Some((
        band,
        Tier {
            system,
            system_label: system.label(),
            score,
            band_lo: band.lo,
            band_hi: band.hi,
            band_label: band.label,
            match_id: r.match_id.clone(),
            at: r.at.clone(),
        },
    ))
}

fn score_series(mine: &[MatchRow]) -> Vec<ScoreSeries> {
    let mut by: BTreeMap<u8, (ScoreSystem, Vec<ScorePoint>)> = BTreeMap::new();
    for r in mine {
        if let Some((sys, v)) = compute::score_of(r) {
            let order = match sys {
                ScoreSystem::Premier => 0,
                ScoreSystem::Competitive => 1,
                ScoreSystem::Wingman => 2,
            };
            by.entry(order).or_insert_with(|| (sys, Vec::new())).1.push(ScorePoint {
                match_id: r.match_id.clone(),
                at: r.at.clone(),
                value: v,
            });
        }
    }
    by.into_values()
        .map(|(system, points)| ScoreSeries {
            system,
            label: system.label(),
            latest: points.last().map(|p| p.value).unwrap_or(0),
            best: points.iter().map(|p| p.value).max().unwrap_or(0),
            points,
        })
        .collect()
}

fn bests(rows: &[&MatchRow]) -> Vec<PersonalBest> {
    // (key, label, metric, value fn, lower is better)
    type F = fn(&MatchRow) -> Option<f64>;
    let defs: [(&'static str, &'static str, &'static str, F, bool); 8] = [
        ("kills", "单场击杀", "kills", |r| Some(r.kills as f64), false),
        ("adr", "单场 ADR", "adr", |r| compute::pair("adr", r).and_then(|a| a.value()), false),
        ("rating", "单场 HLTV 2.0", "rating", |r| (r.rounds > 0).then_some(r.rating), false),
        ("swing", "单场 Swing", "swing", |r| compute::pair("swing", r).and_then(|a| a.value()), false),
        ("headshotKills", "单场爆头击杀", "hsPct", |r| Some(r.headshot_kills as f64), false),
        ("multiKill", "单回合最多击杀", "multiKills", |r| (r.best_multi_kill > 0).then_some(r.best_multi_kill as f64), false),
        ("clutch", "最大残局胜利（1vN）", "clutch", |r| (r.best_clutch_won > 0).then_some(r.best_clutch_won as f64), false),
        ("ttd", "单场最快 TTD", "ttd", |r| compute::pair("ttd", r).and_then(|a| a.value()), true),
    ];
    let mut out = Vec::new();
    for (key, label, metric, f, lower) in defs {
        // Newest wins ties (rows are oldest → newest).
        let mut best: Option<(&MatchRow, f64)> = None;
        for r in rows {
            let Some(v) = f(r) else { continue };
            let better = match best {
                None => true,
                Some((_, b)) => {
                    if lower {
                        v <= b
                    } else {
                        v >= b
                    }
                }
            };
            if better {
                best = Some((r, v));
            }
        }
        if let Some((r, v)) = best {
            out.push(PersonalBest {
                key,
                label,
                metric,
                value: v,
                match_id: r.match_id.clone(),
                map_name: r.map_name.clone(),
                at: r.at.clone(),
                result: result_of(r),
                score_own: r.score_own,
                score_opp: r.score_opp,
            });
        }
    }
    out
}

fn map_rows(rows: &[&MatchRow]) -> Vec<MapRow> {
    let mut by: BTreeMap<String, Vec<&MatchRow>> = BTreeMap::new();
    for r in rows {
        by.entry(r.map_name.clone().unwrap_or_else(|| "未知".into())).or_default().push(r);
    }
    let mut out: Vec<MapRow> = by
        .into_iter()
        .map(|(map_name, rs)| {
            let wins = rs.iter().filter(|r| r.won == Some(true)).count() as u32;
            let losses = rs.iter().filter(|r| r.won == Some(false)).count() as u32;
            let n = rs.len() as u32;
            MapRow {
                map_name,
                matches: n,
                wins,
                losses,
                draws: n - wins - losses,
                win_rate: rate(wins as f64 * 100.0, n as f64),
                rounds: rs.iter().map(|r| r.rounds).sum(),
                adr: compute::aggregate("adr", rs.iter().copied()).value(),
                kast: compute::aggregate("kast", rs.iter().copied()).value(),
                kd: compute::aggregate("kd", rs.iter().copied()).value(),
                rating: compute::aggregate("rating", rs.iter().copied()).value(),
                swing: compute::aggregate("swing", rs.iter().copied()).value(),
            }
        })
        .collect();
    out.sort_by(|a, b| b.matches.cmp(&a.matches).then_with(|| a.map_name.cmp(&b.map_name)));
    out
}

pub fn build_profile(db: &Database, steamid: u64, opts: &ProfileOptions) -> Result<Profile> {
    let c = &db.conn;
    let window = opts.window.unwrap_or(DEFAULT_WINDOW).clamp(1, 200);
    let (person_id, ids) = load::person_steamids(c, steamid)?;
    let id_set: HashSet<u64> = ids.iter().copied().collect();
    let display_name = crate::onboarding::main_player::describe(c, steamid as i64)?.1;

    let all = load::player_rows(c, None)?;
    let (mine_all, others): (Vec<&MatchRow>, Vec<&MatchRow>) = all.iter().partition(|r| id_set.contains(&r.steamid));
    // One row per match even if two identities of the person somehow played the same match.
    let mut seen = HashSet::new();
    let mine_all: Vec<&MatchRow> = mine_all.into_iter().filter(|r| seen.insert(r.match_id.clone())).collect();
    let mine: Vec<&MatchRow> = match opts.last_matches {
        Some(n) if (n as usize) < mine_all.len() => mine_all[mine_all.len() - n as usize..].to_vec(),
        _ => mine_all.clone(),
    };
    let mine_owned: Vec<MatchRow> = mine_all.iter().map(|r| (*r).clone()).collect();

    let tier = find_tier(&mine_owned);
    let tier_pool: Option<Vec<&MatchRow>> = tier.as_ref().map(|(band, _)| {
        others
            .iter()
            .copied()
            .filter(|r| compute::score_of(r).is_some_and(|(s, v)| band.contains(s, v)))
            .collect()
    });

    let wins = mine.iter().filter(|r| r.won == Some(true)).count() as u32;
    let losses = mine.iter().filter(|r| r.won == Some(false)).count() as u32;
    let summary = Summary {
        matches: mine.len() as u32,
        wins,
        losses,
        draws: mine.len() as u32 - wins - losses,
        win_rate: rate(wins as f64 * 100.0, mine.len() as f64),
        rounds: mine.iter().map(|r| r.rounds).sum(),
        kills: mine.iter().map(|r| r.kills).sum(),
        deaths: mine.iter().map(|r| r.deaths).sum(),
        first_match_at: mine.first().map(|r| r.at.clone()),
        last_match_at: mine.last().map(|r| r.at.clone()),
    };

    let recent_rows = &mine[mine.len().saturating_sub(window as usize)..];
    let metrics: Vec<MetricSummary> = METRICS
        .iter()
        .map(|def| {
            let a = compute::aggregate(def.key, mine.iter().copied());
            MetricSummary {
                key: def.key,
                name: def.name,
                unit: def.unit,
                higher_is_better: def.higher_is_better,
                category: def.category,
                value: a.value(),
                recent: compute::aggregate(def.key, recent_rows.iter().copied()).value(),
                matches_with_data: a.matches,
                support: a.support,
                percentile: percentile_for(def, &a, tier_pool.as_deref(), &others),
            }
        })
        .collect();

    let trend = Trend {
        window,
        matches: mine
            .iter()
            .map(|r| TrendMatch { match_id: r.match_id.clone(), at: r.at.clone(), map_name: r.map_name.clone(), result: result_of(r) })
            .collect(),
        series: METRICS
            .iter()
            .map(|def| TrendSeries {
                key: def.key,
                values: mine.iter().map(|r| compute::pair(def.key, r).and_then(|a| a.value())).collect(),
                rolling: compute::rolling(def.key, &mine, window as usize),
            })
            .collect(),
    };

    let match_ids: Vec<String> = mine.iter().map(|r| r.match_id.clone()).collect();
    let sides = load::side_totals(c, &ids, &match_ids)?
        .into_iter()
        .filter(|(side, _)| *side == 2 || *side == 3)
        .map(|(side, t)| SideRow {
            side,
            rounds: t.rounds,
            rounds_won: t.rounds_won,
            round_win_rate: rate(t.rounds_won as f64 * 100.0, t.rounds as f64),
            kills: t.kills,
            deaths: t.deaths,
            kd: if t.rounds == 0 { None } else { Some(t.kills as f64 / t.deaths.max(1) as f64) },
            adr: rate(t.damage as f64, t.rounds as f64),
            kast: rate(t.kast as f64 * 100.0, t.rounds as f64),
            opening_kills: t.opening_kills,
            opening_deaths: t.opening_deaths,
            opening_rate: rate(t.opening_kills as f64 * 100.0, (t.opening_kills + t.opening_deaths) as f64),
            swing: t.swing.and_then(|s| rate(s, t.rounds as f64)),
        })
        .collect();
    let mut weapons: Vec<WeaponRow> = load::weapon_totals(c, &ids, &match_ids)?
        .into_iter()
        .map(|(weapon, t)| WeaponRow {
            weapon,
            kills: t.kills,
            headshot_kills: t.headshot_kills,
            hs_pct: rate(t.headshot_kills as f64 * 100.0, t.kills as f64),
            shots: t.shots,
            hits: t.hits,
            accuracy: rate(t.hits as f64 * 100.0, t.shots as f64),
            head_hit_rate: rate(t.head_hits as f64 * 100.0, t.hits as f64),
        })
        .collect();
    weapons.sort_by(|a, b| b.kills.cmp(&a.kills).then(b.shots.cmp(&a.shots)).then_with(|| a.weapon.cmp(&b.weapon)));

    let mut focus: Vec<FocusItem> = metrics
        .iter()
        .filter_map(|m| {
            let p = m.percentile.value?;
            let value = m.value?;
            (p < FOCUS_BELOW).then(|| FocusItem {
                key: m.key,
                name: m.name,
                value,
                percentile: p,
                sample_median: m.percentile.sample_median,
                scope: m.percentile.scope,
                sample_size: m.percentile.sample_size,
            })
        })
        .collect();
    focus.sort_by(|a, b| a.percentile.partial_cmp(&b.percentile).unwrap_or(std::cmp::Ordering::Equal).then(a.key.cmp(b.key)));
    focus.truncate(MAX_FOCUS);

    Ok(Profile {
        steamid: steamid.to_string(),
        steamids: ids.iter().map(|s| s.to_string()).collect(),
        person_id,
        display_name,
        window,
        last_matches: opts.last_matches,
        summary,
        sample: SampleInfo {
            tier_players: tier_pool.as_deref().map(count_players).unwrap_or(0),
            library_players: count_players(&others),
            min_tier_sample: compute::MIN_TIER_SAMPLE as u32,
            min_sample: compute::MIN_ANY_SAMPLE as u32,
            confident_sample: compute::CONFIDENT_SAMPLE as u32,
        },
        tier: tier.map(|(_, t)| t),
        scores: score_series(&mine_owned),
        metrics,
        trend,
        maps: map_rows(&mine),
        sides,
        weapons,
        bests: bests(&mine),
        focus,
    })
}
