use pbdems2::{DemoParser, PreparedPlayback};

use crate::entity::{ClassInfo, SerializerContainer};
use crate::error::{Error, Result};

use super::{CitadelAdapter, Context, GameEvent, HEADER_SIZE, Parser};

const DEFAULT_TICK_INTERVAL: f32 = 1.0 / 30.0;

impl Parser {
    fn demo_parser(&self) -> Result<DemoParser<'_>> {
        DemoParser::new(self.data()).map_err(Error::from)
    }

    fn prepared(&self) -> Result<&PreparedPlayback<CitadelAdapter>> {
        if let Some(prepared) = self.prepared_cache.get() {
            return Ok(prepared);
        }

        let _guard = self.prepared_lock.lock().map_err(|_| Error::Parse {
            context: "prepared-playback cache lock poisoned".into(),
        })?;
        if self.prepared_cache.get().is_none() {
            let prepared = self
                .demo_parser()?
                .prepare(CitadelAdapter::default(), DEFAULT_TICK_INTERVAL)?;
            self.prepared_cache.set(prepared).unwrap_or_else(|_| {
                unreachable!("prepared-playback cache initialized while locked")
            });
        }
        Ok(self
            .prepared_cache
            .get()
            .expect("prepared-playback cache populated"))
    }

    /// Parse send tables from the signon stream.
    pub fn parse_send_tables(&self) -> Result<SerializerContainer> {
        Ok(self.prepared()?.initial_state().serializers().clone())
    }

    /// Parse network class information from the signon stream.
    pub fn parse_class_info(&self) -> Result<ClassInfo> {
        Ok(self.prepared()?.initial_state().class_info().clone())
    }

    /// Parse all initialization data through `DEM_SyncTick`.
    pub fn parse_init(&self) -> Result<Context> {
        Ok(self.prepared()?.initial_state().clone())
    }

    /// Parse the demo to a specific tick, restoring the nearest full packet.
    pub fn parse_to_tick(&self, target_tick: i32) -> Result<Context> {
        let parser = self.demo_parser()?;
        self.prepared()?.session(parser)?.parse_to_tick(target_tick)
    }

    /// Parse the entire demo, calling a callback once per completed tick.
    pub fn run_to_end<F>(&self, on_tick: F) -> Result<()>
    where
        F: FnMut(&Context),
    {
        let parser = self.demo_parser()?;
        self.prepared()?.session(parser)?.run_to_end(on_tick)?;
        Ok(())
    }

    /// Parse the entire demo. Create only the selected entity classes.
    pub fn run_to_end_filtered<F>(
        &self,
        class_filter: &std::collections::HashSet<&str>,
        on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(&Context),
    {
        let parser = self.demo_parser()?;
        self.prepared()?
            .session(parser)?
            .run_to_end_filtered(class_filter, on_tick)?;
        Ok(())
    }

    /// Byte offsets relative to the post-header stream and ticks of full packets.
    pub fn full_packet_offsets(&self) -> Result<Vec<(usize, i32)>> {
        Ok(self
            .prepared()?
            .index()
            .full_packets()
            .iter()
            .map(|position| (position.offset() - HEADER_SIZE, position.tick()))
            .collect())
    }

    /// Every distinct post-signon tick in ascending stream order.
    pub fn distinct_ticks(&self) -> Result<Vec<i32>> {
        Ok(self.prepared()?.index().distinct_ticks().to_vec())
    }

    /// Decode one full-packet-bounded segment for parallel consumers.
    pub fn decode_segment<F>(
        &self,
        start: Option<usize>,
        end_tick: i32,
        class_filter: &std::collections::HashSet<&str>,
        on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(&Context),
    {
        let parser = self.demo_parser()?;
        self.prepared()?.session(parser)?.decode_segment(
            start.map(|offset| offset + HEADER_SIZE),
            end_tick,
            class_filter,
            on_tick,
        )?;
        Ok(())
    }

    /// Read modifier changes from the packet stream, without keyframe snapshots.
    pub(crate) fn decode_stat_ticks<F>(
        &self,
        end_tick: i32,
        classes: &std::collections::HashSet<&str>,
        on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(&Context),
    {
        let mut session = self.prepared()?.session(self.demo_parser()?)?;
        // Relay keyframes can contain server-side modifier state ahead of the
        // recorded entities. Replay deltas from signon to keep both at one time.
        session.adapter_mut().skip_modifier_snapshots();
        session.decode_segment(None, end_tick, classes, on_tick)?;
        Ok(())
    }

    /// Visit merged modifier changes at each recorded tick.
    ///
    /// Relay keyframe tables can have a different capture time from entities.
    /// This reads packet deltas from signon and does not decode entities.
    /// # Errors
    /// Returns an error if the replay or a packet cannot be decoded.
    pub fn visit_modifier_changes(
        &self,
        mut visit: impl FnMut(i32, crate::ModifierChange),
    ) -> Result<()> {
        let mut state = crate::ModifierState::default();
        self.decode_stat_ticks(i32::MAX, &std::collections::HashSet::new(), |ctx| {
            for change in state.update(ctx) {
                visit(ctx.tick(), change);
            }
        })
    }

    /// Parse entities and only selected final event message types in one pass.
    pub fn run_to_end_with_event_types_filtered<F>(
        &self,
        class_filter: &std::collections::HashSet<&str>,
        event_types: &std::collections::HashSet<u32>,
        mut on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(&Context, &[GameEvent]),
    {
        let parser = self.demo_parser()?;
        let mut session = self.prepared()?.session(parser)?;
        session.adapter_mut().enable_event_types(event_types);
        session.run_to_end_filtered_with_adapter(class_filter, |state, adapter| {
            on_tick(state, adapter.tick_events());
            adapter.clear_tick_events();
        })?;
        Ok(())
    }

    /// Parse entities and game events in one filtered pass.
    pub fn run_to_end_with_events_filtered<F>(
        &self,
        class_filter: &std::collections::HashSet<&str>,
        mut on_tick: F,
    ) -> Result<()>
    where
        F: FnMut(&Context, &[GameEvent]),
    {
        let parser = self.demo_parser()?;
        let mut session = self.prepared()?.session(parser)?;
        session.adapter_mut().enable_events();
        session.run_to_end_filtered_with_adapter(class_filter, |state, adapter| {
            on_tick(state, adapter.tick_events());
            adapter.clear_tick_events();
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires local 106996573.dem"]
    fn relay_keyframes_do_not_apply_future_modifiers() {
        let parser = Parser::from_file(std::path::Path::new("../../106996573.dem")).unwrap();
        let initial = parser.parse_init().unwrap();
        let classes = initial.serializers().iter().map(|(name, _)| name).collect();
        let clock = crate::ModifierClock::resolve(&initial);
        let mut state = crate::EffectiveModifierState::default();
        let mut checked = Vec::new();
        parser
            .decode_stat_ticks(51842, &classes, |ctx| {
                state.update(ctx, clock.game_time(ctx));
                if [49921, 50707, 51841].contains(&ctx.tick()) {
                    // Serial 7713 appears in the 49921 keyframe, but its purchase
                    // occurs at 51841. The earlier two queries must not use it.
                    assert_eq!(state.entries().contains_key(&7713), ctx.tick() == 51841);
                    checked.push(ctx.tick());
                }
            })
            .unwrap();
        assert_eq!(checked, [49921, 50707, 51841]);
    }
}
