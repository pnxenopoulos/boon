use std::sync::Arc;

use super::{CalculationError, StatCatalog};
use crate::{EffectiveModifierState, ModifierClock, Parser, demo::adapter::CitadelAdapter};
use pbdems2::PlaybackCheckpoint;

pub(super) struct StatCheckpoint {
    pub playback: PlaybackCheckpoint<CitadelAdapter>,
    pub modifiers: EffectiveModifierState,
}

/// Optional, bounded exact replay checkpoints for repeated stat batches.
///
/// Preparation is one full packet-delta pass. Queries retain ordinary stat
/// semantics, including modifier expiry and partial refresh history. Drop this
/// value to release checkpoints; it does not retain the encoded demo. Each
/// checkpoint owns decoded state; smaller budgets reduce retained memory.
pub struct StatReplay {
    checkpoints: Vec<StatCheckpoint>,
    demo: Arc<()>,
    catalog: Arc<()>,
}

impl StatReplay {
    pub(super) fn validate(
        &self,
        parser: &Parser,
        catalog: &StatCatalog,
    ) -> Result<(), CalculationError> {
        if !Arc::ptr_eq(&self.demo, parser.replay_identity())
            || !Arc::ptr_eq(&self.catalog, &catalog.identity)
        {
            return Err(CalculationError::Invalid(
                "stat replay belongs to a different parser or catalog".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn before(&self, tick: i32) -> Option<&StatCheckpoint> {
        let index = self
            .checkpoints
            .partition_point(|c| c.playback.state().tick() < tick);
        index.checked_sub(1).map(|i| &self.checkpoints[i])
    }

    /// Number of retained checkpoints (at most 128).
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    /// Whether no gameplay checkpoints were available.
    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }
}

impl Parser {
    /// Prepare evenly spaced exact checkpoints for interactive stat queries.
    ///
    /// `max_checkpoints` bounds retained snapshots, capped at 128. Zero is an
    /// error. The same parser and catalog must be used for subsequent queries.
    /// # Errors
    /// Returns an error if the budget is zero or playback cannot be decoded.
    pub fn prepare_stat_replay(
        &self,
        catalog: &StatCatalog,
        max_checkpoints: usize,
    ) -> Result<StatReplay, CalculationError> {
        if max_checkpoints == 0 {
            return Err(CalculationError::Invalid(
                "provide at least one checkpoint".into(),
            ));
        }
        let ticks = self.distinct_ticks()?;
        let initial = self.parse_init()?;
        let classes = initial.serializers().iter().map(|(name, _)| name).collect();
        let clock = ModifierClock::resolve(&initial);
        let mut modifiers = EffectiveModifierState::with_catalog(catalog);
        modifiers.rebuild(&initial, clock.game_time(&initial));
        let count = max_checkpoints.min(128).min(ticks.len());
        let mut checkpoints: Vec<StatCheckpoint> = Vec::with_capacity(count);
        for i in 0..count {
            let tick = ticks[(i + 1) * ticks.len() / (count + 1)];
            let playback = self.stat_checkpoint(
                checkpoints.last().map(|c| &c.playback),
                tick,
                &classes,
                |ctx| {
                    modifiers.update(ctx, clock.game_time(ctx));
                },
            )?;
            checkpoints.push(StatCheckpoint {
                playback,
                modifiers: modifiers.clone(),
            });
        }
        Ok(StatReplay {
            checkpoints,
            demo: self.replay_identity().clone(),
            catalog: catalog.identity.clone(),
        })
    }
}
