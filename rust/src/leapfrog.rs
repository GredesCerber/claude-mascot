//! Ballistic leapfrog arcs, including the bodies' physical contact envelopes.
use crate::{
    engine::{Pet, GRAVITY},
    Vec2, World,
};
#[derive(Clone, Copy, Debug)]
pub(crate) struct Arc {
    pub launch: f64,
    pub vx: f64,
    pub start_y: f64,
}
pub(crate) fn plan(jumper: &Pet, partner: &Pet, target: Vec2, world: &World) -> Option<Arc> {
    let scale = jumper
        .display_scale
        .max(partner.display_scale)
        .clamp(0.5, 2.0);
    let direction = (target.x - jumper.pos.x).signum();
    let clearance = 67.0 * scale;
    let area = world.area_at(jumper.pos);
    let maximum = (jumper.pos.y - area.y - 55.0 * jumper.display_scale.clamp(0.5, 2.0) - 8.0)
        .min(2050.0_f64.powi(2) / (2.0 * GRAVITY));
    let mut rise = 140.0 * scale;
    let exit_margin = 38.0 * (jumper.display_scale + partner.display_scale) + 8.0 * scale;
    for _ in 0..48 {
        if rise > maximum || !rise.is_finite() {
            return None;
        }
        let launch = (2.0 * GRAVITY * rise).sqrt();
        let root = (launch * launch - 2.0 * GRAVITY * clearance).sqrt();
        let start = (launch - root) / GRAVITY;
        let finish = (launch
            + (launch * launch + 2.0 * GRAVITY * (target.y - jumper.pos.y)).sqrt())
            / GRAVITY;
        let vx = (target.x - jumper.pos.x) / (finish - start);
        let clear_until = (launch + root) / GRAVITY;
        let exit_x = jumper.pos.x + vx * (clear_until - start);
        if vx.is_finite()
            && vx.abs() <= 600.0
            && direction * (exit_x - partner.pos.x) >= exit_margin
        {
            return Some(Arc {
                launch,
                vx,
                start_y: jumper.pos.y - clearance,
            });
        }
        rise *= 1.1;
    }
    None
}
