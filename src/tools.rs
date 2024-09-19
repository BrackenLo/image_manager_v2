//====================================================================

//====================================================================

#[derive(Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[inline]
    pub fn _new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[inline]
    pub fn _from_size(width: f32, height: f32) -> Self {
        Self {
            x: 0.,
            y: 0.,
            width,
            height,
        }
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        }
    }
}

//====================================================================

pub fn aabb_point(point: glam::Vec2, area_pos: glam::Vec2, area_size: glam::Vec2) -> bool {
    let dx = point.x - area_pos.x;
    let px = area_size.x / 2. - dx.abs();

    if px <= 0. {
        return false;
    }

    let dy = point.y - area_pos.y;
    let py = area_size.y / 2. - dy.abs();

    if py <= 0. {
        return false;
    }

    true
}

// pub(crate) fn aabb(
//     pos_a: glam::Vec2,
//     size_a: glam::Vec2,
//     pos_b: glam::Vec2,
//     size_b: glam::Vec2,
// ) -> bool {
//     let half_a = glam::vec2(size_a.x / 2., size_a.y / 2.);
//     let half_b = glam::vec2(size_b.x / 2., size_b.y / 2.);

//     let a_min_x = pos_a.x - half_a.x;
//     let a_max_x = pos_a.x + half_a.x;

//     let b_min_x = pos_b.x - half_b.x;
//     let b_max_x = pos_b.x + half_b.x;

//     let a_min_y = pos_a.y - half_a.y;
//     let a_max_y = pos_a.y + half_a.y;

//     let b_min_y = pos_b.y - half_b.y;
//     let b_max_y = pos_b.y + half_b.y;

//     a_min_x <= b_max_x && a_max_x >= b_min_x && a_min_y <= b_max_y && a_max_y >= b_min_y
// }

//====================================================================
