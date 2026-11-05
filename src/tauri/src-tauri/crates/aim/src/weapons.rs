//! Weapon constants (CS2 values): fire cycle and movement speed.

/// Accurate-movement threshold: inaccuracy is at its minimum below 34 % of the weapon's max speed.
pub const ACCURATE_SPEED_FRACTION: f32 = 0.34;

#[derive(Debug, Clone, Copy)]
pub struct WeaponInfo {
    pub name: &'static str,
    /// Seconds between shots at full auto / max fire rate.
    pub cycle_s: f32,
    pub max_speed: f32,
    /// Max speed while scoped (snipers, AUG / SG 553).
    pub scoped_speed: Option<f32>,
    /// Spray-control analysis is done for this weapon.
    pub spray: bool,
}

const fn w(name: &'static str, cycle_s: f32, max_speed: f32, scoped_speed: Option<f32>, spray: bool) -> WeaponInfo {
    WeaponInfo { name, cycle_s, max_speed, scoped_speed, spray }
}

/// Guns only: knives, grenades and the Zeus are not part of aim stats.
pub const WEAPONS: &[WeaponInfo] = &[
    // Spray-analysed automatics (docs/roadmap.md 2.4)
    w("ak47", 0.100, 215.0, None, true),
    w("m4a1", 0.090, 225.0, None, true), // M4A4
    w("m4a1_silencer", 0.100, 225.0, None, true),
    w("galilar", 0.090, 215.0, None, true),
    w("famas", 0.090, 220.0, None, true),
    w("mp9", 0.070, 240.0, None, true),
    w("mac10", 0.075, 240.0, None, true),
    // Other rifles / snipers
    w("aug", 0.090, 220.0, Some(150.0), false),
    w("sg556", 0.090, 210.0, Some(150.0), false),
    w("awp", 1.455, 200.0, Some(100.0), false),
    w("ssg08", 1.250, 230.0, Some(230.0), false),
    w("scar20", 0.250, 215.0, Some(120.0), false),
    w("g3sg1", 0.250, 215.0, Some(120.0), false),
    // SMGs / heavy
    w("mp7", 0.080, 220.0, None, false),
    w("mp5sd", 0.080, 235.0, None, false),
    w("ump45", 0.090, 230.0, None, false),
    w("p90", 0.070, 230.0, None, false),
    w("bizon", 0.080, 240.0, None, false),
    w("negev", 0.075, 150.0, None, false),
    w("m249", 0.080, 195.0, None, false),
    w("nova", 0.880, 220.0, None, false),
    w("xm1014", 0.350, 215.0, None, false),
    w("mag7", 0.850, 225.0, None, false),
    w("sawedoff", 0.850, 210.0, None, false),
    // Pistols
    w("glock", 0.150, 240.0, None, false),
    w("hkp2000", 0.170, 240.0, None, false),
    w("usp_silencer", 0.170, 240.0, None, false),
    w("p250", 0.150, 240.0, None, false),
    w("elite", 0.120, 240.0, None, false),
    w("fiveseven", 0.150, 240.0, None, false),
    w("tec9", 0.120, 240.0, None, false),
    w("cz75a", 0.100, 240.0, None, false),
    w("deagle", 0.225, 230.0, None, false),
    w("revolver", 0.500, 220.0, None, false),
];

/// Weapons whose sprays are analysed, in display order.
pub const SPRAY_WEAPONS: &[&str] = &["ak47", "m4a1", "m4a1_silencer", "galilar", "famas", "mp9", "mac10"];

/// Normalize a demo weapon name to a [`WEAPONS`] entry. Handles `weapon_` prefixes and platform
/// skins such as `5e_2023pass1_usp_silencer` (longest `_<name>` suffix wins).
pub fn canonical(raw: &str) -> Option<&'static WeaponInfo> {
    let s = raw.strip_prefix("weapon_").unwrap_or(raw);
    if let Some(w) = WEAPONS.iter().find(|w| w.name == s) {
        return Some(w);
    }
    WEAPONS
        .iter()
        .filter(|w| s.len() > w.name.len() && s.ends_with(w.name) && s.as_bytes()[s.len() - w.name.len() - 1] == b'_')
        .max_by_key(|w| w.name.len())
}

impl WeaponInfo {
    pub fn cycle_ticks(&self, tick_rate: f32) -> f32 {
        self.cycle_s * tick_rate
    }
    /// Speed at or below which a shot is fully accurate.
    pub fn accurate_speed(&self, scoped: bool) -> f32 {
        let max = if scoped { self.scoped_speed.unwrap_or(self.max_speed) } else { self.max_speed };
        max * ACCURATE_SPEED_FRACTION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_names() {
        assert_eq!(canonical("ak47").unwrap().name, "ak47");
        assert_eq!(canonical("weapon_m4a1_silencer").unwrap().name, "m4a1_silencer");
        assert_eq!(canonical("5e_2023pass1_usp_silencer").unwrap().name, "usp_silencer");
        assert_eq!(canonical("m4a1").unwrap().name, "m4a1");
        assert!(canonical("knife_karambit").is_none());
        assert!(canonical("hegrenade").is_none());
        assert!(canonical("taser").is_none());
    }
}
