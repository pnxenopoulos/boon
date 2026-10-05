//! Deadlock world coordinates for City Never Sleeps demos and later.

pub use pbdems2::position::{CELL_BITS, CELL_SIZE, cell_to_world_with_world_half};

/// Half the extent of Deadlock's 128-cell world grid, in Hammer units.
pub const WORLD_HALF: f32 = 32768.0;

/// Combine a cell index and in-cell offset into a Deadlock world coordinate.
pub fn cell_to_world(cell: i32, offset: f32) -> f32 {
    cell_to_world_with_world_half(cell, offset, WORLD_HALF)
}

/// Read an entity's split position using Deadlock's world grid.
///
/// Pass resolved cell and offset keys for X, Y and Z. Missing fields default to
/// zero through the entity accessors; verify the keys before relying on them.
pub fn world_position(
    entity: &crate::Entity,
    cell_keys: [Option<u64>; 3],
    offset_keys: [Option<u64>; 3],
) -> [f32; 3] {
    std::array::from_fn(|i| {
        cell_to_world(
            entity.get_i64(cell_keys[i]) as i32,
            entity.get_f32(offset_keys[i]),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Entity, FieldValue};

    #[test]
    fn entity_position_uses_deadlock_origin_on_all_axes() {
        let mut entity =
            Entity::from_fields(1, 1, 1, "CCitadelPlayerPawn", true, Default::default()).unwrap();
        for (key, value) in [(1, 64), (2, 63), (3, 65)] {
            entity.fields.insert(key, FieldValue::I32(value));
        }
        for (key, value) in [(4, 0.0), (5, 511.0), (6, 128.0)] {
            entity.fields.insert(key, FieldValue::F32(value));
        }
        assert_eq!(
            world_position(
                &entity,
                [Some(1), Some(2), Some(3)],
                [Some(4), Some(5), Some(6)]
            ),
            [0.0, -1.0, 640.0]
        );
    }
}
