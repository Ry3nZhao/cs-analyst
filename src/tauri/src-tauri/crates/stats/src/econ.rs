//! Economy helpers: buy-type classification and grenade inventory approximation.

use serde::{Deserialize, Serialize};

/// Team buy type of a round, from the average equipment value per player (freeze-end snapshot
/// plus purchases made after freeze end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyType {
    /// First round of each half in regulation.
    Pistol,
    /// Average < $2000.
    Eco,
    /// Between eco and full buy.
    Force,
    /// Average ≥ $3500 (T) / $4000 (CT).
    Full,
}

pub const ECO_MAX_AVG: u32 = 2000;
pub const FULL_MIN_AVG_T: u32 = 3500;
pub const FULL_MIN_AVG_CT: u32 = 4000;

impl BuyType {
    pub fn as_str(self) -> &'static str {
        match self {
            BuyType::Pistol => "pistol",
            BuyType::Eco => "eco",
            BuyType::Force => "force",
            BuyType::Full => "full",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pistol" => BuyType::Pistol,
            "eco" => BuyType::Eco,
            "force" => BuyType::Force,
            "full" => BuyType::Full,
            _ => return None,
        })
    }

    pub fn classify(pistol_round: bool, side: u8, total_equip: u32, players: u32) -> Self {
        if pistol_round {
            return BuyType::Pistol;
        }
        let avg = if players > 0 { total_equip / players } else { 0 };
        let full = if side == demo_parse::TEAM_CT { FULL_MIN_AVG_CT } else { FULL_MIN_AVG_T };
        if avg < ECO_MAX_AVG {
            BuyType::Eco
        } else if avg < full {
            BuyType::Force
        } else {
            BuyType::Full
        }
    }
}

/// Grenade kinds as named by `grenades.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nade {
    Flash,
    Smoke,
    He,
    /// Molotov or incendiary (both detonate as `inferno`).
    Fire,
    Decoy,
}

impl Nade {
    /// From an `item_purchase` item name (e.g. `Smoke Grenade`, `High Explosive Grenade`).
    pub fn from_item(item: &str) -> Option<(Nade, u32)> {
        let s = item.to_ascii_lowercase();
        let s = s.trim_start_matches("weapon_");
        Some(match s {
            "flashbang" => (Nade::Flash, 200),
            "smoke grenade" | "smokegrenade" => (Nade::Smoke, 300),
            "high explosive grenade" | "hegrenade" => (Nade::He, 300),
            "molotov" => (Nade::Fire, 400),
            "incendiary grenade" | "incgrenade" => (Nade::Fire, 500),
            "decoy grenade" | "decoy" => (Nade::Decoy, 50),
            _ => return None,
        })
    }

    /// From a `grenades.kind` detonation.
    pub fn from_kind(kind: &str) -> Option<Nade> {
        Some(match kind {
            "flashbang" => Nade::Flash,
            "smokegrenade" => Nade::Smoke,
            "hegrenade" => Nade::He,
            "inferno" => Nade::Fire,
            "decoy" => Nade::Decoy,
            _ => return None,
        })
    }
}

/// Grenades a player is believed to hold (purchases − throws, carried over while alive).
/// Pick-ups and drops are not visible in the events, so this is an approximation.
#[derive(Debug, Clone, Default)]
pub struct NadeInventory {
    /// (kind, value) in acquisition order.
    items: Vec<(Nade, u32)>,
}

impl NadeInventory {
    pub fn buy(&mut self, nade: Nade, value: u32) {
        self.items.push((nade, value));
    }
    pub fn sell(&mut self, nade: Nade) {
        if let Some(i) = self.items.iter().rposition(|(n, _)| *n == nade) {
            self.items.remove(i);
        }
    }
    pub fn throw(&mut self, nade: Nade) {
        if let Some(i) = self.items.iter().position(|(n, _)| *n == nade) {
            self.items.remove(i);
        }
    }
    pub fn value(&self) -> u32 {
        self.items.iter().map(|(_, v)| v).sum()
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_buys() {
        assert_eq!(BuyType::classify(true, 2, 50_000, 5), BuyType::Pistol);
        assert_eq!(BuyType::classify(false, 2, 5 * 1000, 5), BuyType::Eco);
        assert_eq!(BuyType::classify(false, 2, 5 * 3000, 5), BuyType::Force);
        assert_eq!(BuyType::classify(false, 2, 5 * 3600, 5), BuyType::Full);
        assert_eq!(BuyType::classify(false, 3, 5 * 3600, 5), BuyType::Force);
        assert_eq!(BuyType::classify(false, 3, 5 * 4500, 5), BuyType::Full);
    }

    #[test]
    fn inventory() {
        let mut inv = NadeInventory::default();
        inv.buy(Nade::Smoke, 300);
        inv.buy(Nade::Flash, 200);
        inv.buy(Nade::Flash, 200);
        inv.throw(Nade::Flash);
        assert_eq!(inv.value(), 500);
        inv.sell(Nade::Smoke);
        assert_eq!(inv.value(), 200);
        inv.throw(Nade::He); // unknown origin: ignored
        assert_eq!(inv.value(), 200);
        assert_eq!(Nade::from_item("Incendiary Grenade"), Some((Nade::Fire, 500)));
    }
}
