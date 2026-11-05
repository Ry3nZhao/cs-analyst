//! Highlight / mistake detection (W3-A, docs/roadmap.md 2.9, docs/highlights.md).
//!
//! Pure functions: [`detect`] takes one match's stored events ([`MatchInput`]) and returns every
//! moment matched by a rule. Scores are 0–100 per rule; the user's per-rule weight and on/off
//! switch are applied when querying, so changing them never needs a recompute. Changing any rule
//! or threshold here must bump [`RULES_VERSION`] — the app then recomputes stored highlights.

mod model;
mod rules;

#[cfg(test)]
mod tests;

pub use model::*;
use serde::{Deserialize, Serialize};

/// Bump whenever a rule, threshold or score changes.
pub const RULES_VERSION: i32 = 1;

macro_rules! kinds {
    ($( $v:ident = $s:literal, $cat:ident, $w:literal; )*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum Kind { $( $v, )* }

        impl Kind {
            pub const ALL: &'static [Kind] = &[ $( Kind::$v, )* ];
            pub fn as_str(self) -> &'static str {
                match self { $( Kind::$v => $s, )* }
            }
            pub fn parse(s: &str) -> Option<Kind> {
                match s { $( $s => Some(Kind::$v), )* _ => None }
            }
            pub fn category(self) -> Category {
                match self { $( Kind::$v => Category::$cat, )* }
            }
            /// Default ranking weight (the user can change it in settings).
            pub fn default_weight(self) -> f64 {
                match self { $( Kind::$v => $w, )* }
            }
        }
    };
}

kinds! {
    // ── 高光 ──
    MultiKill = "multi_kill", Highlight, 1.0;
    QuickKills = "quick_kills", Highlight, 0.8;
    ClutchWin = "clutch_win", Highlight, 1.0;
    FlickHeadshot = "flick_headshot", Highlight, 0.9;
    Wallbang = "wallbang", Highlight, 0.8;
    SmokeKill = "smoke_kill", Highlight, 0.7;
    BlindKill = "blind_kill", Highlight, 0.8;
    AirborneKill = "airborne_kill", Highlight, 0.7;
    NoscopeKill = "noscope_kill", Highlight, 0.9;
    SwingKill = "swing_kill", Highlight, 0.9;
    KeyDefuse = "key_defuse", Highlight, 0.8;
    NinjaDefuse = "ninja_defuse", Highlight, 1.0;
    // ── 失误 ──
    UntradedOpeningDeath = "untraded_opening_death", Mistake, 1.0;
    UnusedUtilityDeath = "unused_utility_death", Mistake, 0.6;
    TeamFlash = "team_flash", Mistake, 0.8;
    TeamDamage = "team_damage", Mistake, 0.9;
    LostHpDuel = "lost_hp_duel", Mistake, 1.0;
    WhiffedSpray = "whiffed_spray", Mistake, 0.9;
    SwingLoss = "swing_loss", Mistake, 1.0;
}

/// Which rules can produce results for this match (e.g. no blind data → no team flashes,
/// no aim analysis → no flick headshots).
pub fn available_kinds(input: &MatchInput) -> Vec<Kind> {
    Kind::ALL
        .iter()
        .copied()
        .filter(|k| match k {
            Kind::FlickHeadshot => input.engagements.is_some(),
            Kind::TeamFlash => input.blind_data,
            Kind::SwingKill | Kind::SwingLoss => !input.win_prob.is_empty(),
            _ => true,
        })
        .collect()
}

/// Run every rule. Results are sorted by (round, start tick, kind, steamid).
pub fn detect(input: &MatchInput) -> Vec<Highlight> {
    let ctx = rules::Ctx::new(input);
    let mut out = Vec::new();
    rules::multi_kill(&ctx, &mut out);
    rules::quick_kills(&ctx, &mut out);
    rules::clutch_win(&ctx, &mut out);
    rules::flick_headshot(&ctx, &mut out);
    rules::special_kills(&ctx, &mut out);
    rules::swing_events(&ctx, &mut out);
    rules::defuses(&ctx, &mut out);
    rules::untraded_opening_death(&ctx, &mut out);
    rules::unused_utility_death(&ctx, &mut out);
    rules::team_flash(&ctx, &mut out);
    rules::team_damage(&ctx, &mut out);
    rules::lost_hp_duel(&ctx, &mut out);
    rules::whiffed_spray(&ctx, &mut out);
    out.sort_by(|a, b| (a.round, a.start_tick, a.kind, a.steamid).cmp(&(b.round, b.start_tick, b.kind, b.steamid)));
    out
}
