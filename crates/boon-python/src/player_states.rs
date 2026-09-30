use crate::*;
use boon_parser::player_states::{DecodedStates, PlayerStateQuery, StateCatalog};

struct StateColumns {
    names: ListStringChunkedBuilder,
    unknown: ListPrimitiveChunkedBuilder<UInt32Type>,
}
impl StateColumns {
    fn new(name: &str, unknown: &str) -> Self {
        Self {
            names: ListStringChunkedBuilder::new(name.into(), 0, 0),
            unknown: ListPrimitiveChunkedBuilder::new(unknown.into(), 0, 0, DataType::UInt32),
        }
    }
    fn append(&mut self, decoded: Option<&DecodedStates>) {
        if let Some(decoded) = decoded {
            self.names
                .append_values_iter(decoded.names.iter().map(AsRef::as_ref));
            self.unknown.append_slice(&decoded.unknown);
        } else {
            self.names.append_null();
            self.unknown.append_null();
        }
    }
    fn columns(&mut self) -> [Column; 2] {
        [
            self.names.finish().into_column(),
            self.unknown.finish().into_column(),
        ]
    }
}

#[pymethods]
impl Demo {
    #[pyo3(signature = (directory, *, ticks=None, steam_ids=None))]
    fn _player_states(
        &self,
        py: Python<'_>,
        directory: PathBuf,
        ticks: Option<Vec<i32>>,
        steam_ids: Option<Vec<u64>>,
    ) -> PyResult<PyDataFrame> {
        py.detach(|| {
            let catalog = StateCatalog::from_directory(&directory)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            let mut query = PlayerStateQuery::default();
            if let Some(ticks) = ticks {
                query = query.ticks(ticks);
            }
            if let Some(ids) = steam_ids {
                query = query.steam_ids(ids);
            }
            let mut ticks = Vec::new();
            let mut steam_ids = Vec::new();
            let mut heroes = Vec::new();
            let mut states = StateColumns::new("states", "unknown_states");
            let mut enabled = StateColumns::new("enabled_states", "unknown_enabled_states");
            let mut disabled = StateColumns::new("disabled_states", "unknown_disabled_states");
            self.parser
                .visit_player_states(&query, &catalog, |row| {
                    ticks.push(row.tick);
                    steam_ids.push(row.steam_id);
                    heroes.push(row.hero_id);
                    states.append(row.states.as_deref());
                    enabled.append(row.enabled_states.as_deref());
                    disabled.append(row.disabled_states.as_deref());
                })
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            let mut columns = vec![
                Column::new("tick".into(), ticks),
                Column::new("steam_id".into(), steam_ids),
                Column::new("hero_id".into(), heroes),
            ];
            columns.extend(states.columns());
            columns.extend(enabled.columns());
            columns.extend(disabled.columns());
            df_from_columns(columns)
                .map(PyDataFrame)
                .map_err(|e| InvalidDemoError::new_err(e.to_string()))
        })
    }
}
