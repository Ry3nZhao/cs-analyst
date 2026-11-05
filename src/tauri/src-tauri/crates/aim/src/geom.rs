//! View-angle geometry. Source conventions: pitch is positive *down*, yaw is positive to the
//! *left* (counter-clockwise seen from above). Every offset this crate outputs is converted to
//! screen terms instead: **v > 0 = up, h > 0 = right**, in degrees.

use demo_parse::Vec3;

pub fn wrap180(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a < -180.0 {
        a += 360.0;
    }
    a
}

/// Pitch / yaw (Source conventions) of the direction from `from` to `to`.
pub fn angles_to(from: Vec3, to: Vec3) -> (f32, f32) {
    let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
    let pitch = -dz.atan2(dx.hypot(dy)).to_degrees();
    let yaw = dy.atan2(dx).to_degrees();
    (pitch, yaw)
}

pub fn distance(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

fn dir(pitch: f32, yaw: f32) -> [f32; 3] {
    let (p, y) = (pitch.to_radians(), yaw.to_radians());
    [p.cos() * y.cos(), p.cos() * y.sin(), -p.sin()]
}

/// Where the aim direction (`pitch`, `yaw`) points relative to the `target` direction, in screen
/// terms: (v, h, total) — v > 0 aim above target, h > 0 aim right of target, total = true angle.
pub fn offset(pitch: f32, yaw: f32, target_pitch: f32, target_yaw: f32) -> (f32, f32, f32) {
    let v = target_pitch - pitch;
    let h = wrap180(target_yaw - yaw);
    let (a, b) = (dir(pitch, yaw), dir(target_pitch, target_yaw));
    let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
    (v, h, dot.acos().to_degrees())
}

/// Normalize a pitch that may come as 0..360.
pub fn norm_pitch(p: f32) -> f32 {
    if p > 180.0 {
        p - 360.0
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offsets_follow_screen_convention() {
        // Aim straight ahead (east); target slightly up and to the left.
        let eye = Vec3 { x: 0.0, y: 0.0, z: 64.0 };
        let head = Vec3 { x: 1000.0, y: 50.0, z: 100.0 };
        let (tp, ty) = angles_to(eye, head);
        assert!(tp < 0.0 && ty > 0.0);
        let (v, h, total) = offset(0.0, 0.0, tp, ty);
        // Crosshair is below and right of the head.
        assert!(v < 0.0 && h > 0.0, "v={v} h={h}"); // crosshair below and right of the head
        assert!((total - (v * v + h * h).sqrt()).abs() < 0.05);
        assert!((wrap180(350.0) + 10.0).abs() < 1e-4);
        assert!((wrap180(-190.0) - 170.0).abs() < 1e-4);
    }
}
