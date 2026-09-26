//! Rift lane identification from observed map locations.

/// Known Rift ("Koth") cash-in sites in the form `([x, y], lane)`.
///
/// Rift entities do not contain `m_iLane`. Boon gets the lane from the cash-in
/// location. We compared each site with the lane of the buffed troopers that
/// spawn after a capture. We observed only these two sites. Other locations
/// resolve to lane `0`.
const RIFT_LANE_SITES: &[([f32; 2], i64)] = &[([-7560.0, 0.0], 1), ([7612.0, 0.0], 6)];

/// Match radius, in Hammer units, for a known Rift site.
/// The two sites are approximately 15,000 units apart.
/// This radius permits location variation but cannot match both sites.
const RIFT_LANE_TOLERANCE: f32 = 1024.0;

/// Maximum valid map coordinate in Hammer units.
///
/// After a Rift ends, the game sets `m_vKothCashInCurrentLocation` to
/// `FLT_MAX`. This value is finite. Therefore, `is_finite` does not reject it.
/// This maximum rejects the value.
pub const RIFT_COORD_SANITY: f32 = 1.0e6;

/// The lane for a Rift cash-in location, or `0` when the location is not a
/// known Rift site (see the observed sites above).
pub fn rift_lane_for(x: f32, y: f32) -> i64 {
    if !x.is_finite() || !y.is_finite() {
        return 0;
    }
    for ([sx, sy], lane) in RIFT_LANE_SITES {
        if (x - sx).abs() <= RIFT_LANE_TOLERANCE && (y - sy).abs() <= RIFT_LANE_TOLERANCE {
            return *lane;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::rift_lane_for;

    #[test]
    fn identifies_observed_sites_and_rejects_unknown_locations() {
        assert_eq!(rift_lane_for(-7560.0, 0.0), 1);
        assert_eq!(rift_lane_for(7612.0, 0.0), 6);
        assert_eq!(rift_lane_for(-6536.0, 1024.0), 1);
        assert_eq!(rift_lane_for(-6535.0, 1024.0), 0);
        assert_eq!(rift_lane_for(0.0, 0.0), 0);
        assert_eq!(rift_lane_for(f32::MAX, 0.0), 0);
        assert_eq!(rift_lane_for(f32::NAN, 0.0), 0);
        assert_eq!(rift_lane_for(0.0, f32::INFINITY), 0);
    }
}
