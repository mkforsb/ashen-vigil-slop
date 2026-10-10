//! The Warden Demon rig: a squat, pot-bellied giant with stubby wings, a tail
//! and a stone club in its right hand. About 5 m tall.

use ashen_engine::math::Vec3;
use ashen_engine::skeleton::Skeleton;

pub const PELVIS: usize = 0;
pub const BELLY: usize = 1;
pub const CHEST: usize = 2;
pub const HEAD: usize = 3;
pub const JAW: usize = 4;
pub const L_UPPER: usize = 5;
pub const L_FORE: usize = 6;
pub const L_HAND: usize = 7;
pub const R_UPPER: usize = 8;
pub const R_FORE: usize = 9;
pub const R_HAND: usize = 10;
pub const L_THIGH: usize = 11;
pub const L_SHIN: usize = 12;
pub const L_FOOT: usize = 13;
pub const R_THIGH: usize = 14;
pub const R_SHIN: usize = 15;
pub const R_FOOT: usize = 16;
pub const TAIL1: usize = 17;
pub const TAIL2: usize = 18;
pub const TAIL3: usize = 19;
pub const L_WING: usize = 20;
pub const R_WING: usize = 21;
pub const BONES: usize = 22;

pub const PELVIS_Y: f32 = 2.3;
pub const UPPER_LEN: f32 = 1.3;
pub const FORE_LEN: f32 = 1.2;
pub const THIGH_LEN: f32 = 1.0;
pub const SHIN_LEN: f32 = 1.0;
/// Length of the club from the fist to the far end of its head.
pub const CLUB_LEN: f32 = 3.4;

pub fn skeleton() -> Skeleton {
    let mut s = Skeleton::new();
    let pelvis = s.add("pelvis", None, Vec3::new(0.0, PELVIS_Y, 0.0));
    let belly = s.add("belly", Some(pelvis), Vec3::new(0.0, 0.5, 0.0));
    let chest = s.add("chest", Some(belly), Vec3::new(0.0, 1.2, 0.2));
    let head = s.add("head", Some(chest), Vec3::new(0.0, 0.6, 0.6));
    s.add("jaw", Some(head), Vec3::new(0.0, -0.1, 0.3));
    let lu = s.add("l_upper", Some(chest), Vec3::new(1.3, 0.3, 0.1));
    let lf = s.add("l_fore", Some(lu), Vec3::new(0.0, -UPPER_LEN, 0.0));
    s.add("l_hand", Some(lf), Vec3::new(0.0, -FORE_LEN, 0.0));
    let ru = s.add("r_upper", Some(chest), Vec3::new(-1.3, 0.3, 0.1));
    let rf = s.add("r_fore", Some(ru), Vec3::new(0.0, -UPPER_LEN, 0.0));
    s.add("r_hand", Some(rf), Vec3::new(0.0, -FORE_LEN, 0.0));
    let lt = s.add("l_thigh", Some(pelvis), Vec3::new(0.75, -0.15, 0.0));
    let ls = s.add("l_shin", Some(lt), Vec3::new(0.0, -THIGH_LEN, 0.0));
    s.add("l_foot", Some(ls), Vec3::new(0.0, -SHIN_LEN, 0.0));
    let rt = s.add("r_thigh", Some(pelvis), Vec3::new(-0.75, -0.15, 0.0));
    let rs = s.add("r_shin", Some(rt), Vec3::new(0.0, -THIGH_LEN, 0.0));
    s.add("r_foot", Some(rs), Vec3::new(0.0, -SHIN_LEN, 0.0));
    let t1 = s.add("tail1", Some(pelvis), Vec3::new(0.0, -0.2, -1.0));
    let t2 = s.add("tail2", Some(t1), Vec3::new(0.0, 0.0, -1.2));
    s.add("tail3", Some(t2), Vec3::new(0.0, 0.0, -1.1));
    s.add("l_wing", Some(chest), Vec3::new(0.6, 0.5, -0.7));
    s.add("r_wing", Some(chest), Vec3::new(-0.6, 0.5, -0.7));
    assert_eq!(s.len(), BONES);
    s
}
