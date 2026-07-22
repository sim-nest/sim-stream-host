//! Stateful, byte-budgeted `scene/glance` diff scheduling.

use std::collections::{BTreeMap, BTreeSet};

use sim_kernel::{Error, Expr, Result};
use sim_lib_scene::GlanceCard;

use crate::lua_render::{LuaCell, LuaCellPriority, LuaRegion};

/// Maximum Lua protocol bytes that may be emitted during one display tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LuaFrameBudget {
    /// Hard byte ceiling for one tick.
    pub max_bytes_per_tick: u32,
}

impl LuaFrameBudget {
    /// Builds a nonzero frame budget.
    pub fn new(max_bytes_per_tick: u32) -> Result<Self> {
        if max_bytes_per_tick == 0 {
            return Err(Error::Eval(
                "Halo Lua frame budget must be greater than zero".to_owned(),
            ));
        }
        Ok(Self { max_bytes_per_tick })
    }
}

/// One tick of changed cells and the work deferred by the byte budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameDiff {
    /// Cells emitted during this tick, in scheduling order.
    pub cells: Vec<LuaCell>,
    /// Exact encoded byte count for `cells`.
    pub bytes: u32,
    /// Changed cells left for a later tick.
    pub deferred: Vec<LuaCell>,
}

impl FrameDiff {
    /// Reports whether the requested frame is fully committed.
    pub fn is_complete(&self) -> bool {
        self.deferred.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CellKey {
    region: LuaRegion,
    index: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateCell {
    glyph: char,
    priority: LuaCellPriority,
}

/// Last cells known to be committed on the Halo display.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LuaFrameState {
    cells: BTreeMap<CellKey, StateCell>,
}

impl LuaFrameState {
    /// Builds an empty display state.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Converts a canonical `scene/glance` expression to display cells.
    pub fn from_glance(frame: &Expr) -> Result<Self> {
        let card = GlanceCard::from_scene(frame)?;
        let mut state = Self::empty();
        let content_priority = if card.bypass_budget {
            LuaCellPriority::Warrant
        } else {
            LuaCellPriority::Normal
        };
        insert_text(
            &mut state.cells,
            LuaRegion::Title,
            &card.title,
            content_priority,
        )?;
        if let Some(metric) = card.metric {
            insert_text(
                &mut state.cells,
                LuaRegion::MetricLabel,
                &metric.label,
                content_priority,
            )?;
            insert_text(
                &mut state.cells,
                LuaRegion::MetricValue,
                &metric.value,
                content_priority,
            )?;
        }
        if let Some(action) = card.action {
            insert_text(
                &mut state.cells,
                LuaRegion::ActionLabel,
                &action.label,
                content_priority,
            )?;
        }
        let urgency_priority = if card.bypass_budget || card.urgency == "warrant" {
            LuaCellPriority::Warrant
        } else if matches!(card.urgency.as_str(), "warn" | "error" | "critical") {
            LuaCellPriority::Urgent
        } else {
            LuaCellPriority::Normal
        };
        insert_text(
            &mut state.cells,
            LuaRegion::Urgency,
            &card.urgency,
            urgency_priority,
        )?;
        if card.bypass_budget {
            state.cells.insert(
                CellKey {
                    region: LuaRegion::Warrant,
                    index: 0,
                },
                StateCell {
                    glyph: '!',
                    priority: LuaCellPriority::Warrant,
                },
            );
        }
        Ok(state)
    }

    /// Number of committed glyph cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }
}

/// Stateful scheduler that applies only emitted cells to its committed state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LuaFrameScheduler {
    committed: LuaFrameState,
}

impl LuaFrameScheduler {
    /// Builds a scheduler with no committed cells.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Builds a scheduler from the last fully committed `scene/glance` frame.
    pub fn from_glance(previous: &Expr) -> Result<Self> {
        Ok(Self {
            committed: LuaFrameState::from_glance(previous)?,
        })
    }

    /// Returns the cells currently committed on the display.
    pub fn committed(&self) -> &LuaFrameState {
        &self.committed
    }

    /// Schedules changed cells for one tick and commits only those that fit.
    pub fn schedule(&mut self, next: &Expr, budget: &LuaFrameBudget) -> Result<FrameDiff> {
        if budget.max_bytes_per_tick == 0 {
            return Err(Error::Eval(
                "Halo Lua frame budget must be greater than zero".to_owned(),
            ));
        }
        let target = LuaFrameState::from_glance(next)?;
        let mut changed = changed_cells(&self.committed, &target);
        changed.sort_by_key(|cell| (cell.priority(), cell.region(), cell.index()));

        let mut cells = Vec::new();
        let mut deferred = Vec::new();
        let mut bytes = 0u32;
        for cell in changed {
            let cell_bytes = cell.byte_len();
            if bytes.saturating_add(cell_bytes) <= budget.max_bytes_per_tick {
                bytes += cell_bytes;
                apply_cell(&mut self.committed, &cell);
                cells.push(cell);
            } else {
                deferred.push(cell);
            }
        }
        if cells.is_empty() && !deferred.is_empty() {
            return Err(Error::HostError(format!(
                "Halo Lua frame budget {} cannot fit the smallest changed cell ({} bytes)",
                budget.max_bytes_per_tick,
                deferred.iter().map(LuaCell::byte_len).min().unwrap_or(0)
            )));
        }
        Ok(FrameDiff {
            cells,
            bytes,
            deferred,
        })
    }
}

/// Computes one budgeted diff from a fully committed previous frame.
pub fn diff_glance(previous: &Expr, next: &Expr, budget: &LuaFrameBudget) -> Result<FrameDiff> {
    LuaFrameScheduler::from_glance(previous)?.schedule(next, budget)
}

fn insert_text(
    cells: &mut BTreeMap<CellKey, StateCell>,
    region: LuaRegion,
    text: &str,
    priority: LuaCellPriority,
) -> Result<()> {
    for (index, glyph) in text.chars().enumerate() {
        let index = u16::try_from(index).map_err(|_| {
            Error::HostError(format!(
                "Halo Lua region {} exceeds {} glyph cells",
                region.token(),
                u16::MAX
            ))
        })?;
        cells.insert(CellKey { region, index }, StateCell { glyph, priority });
    }
    Ok(())
}

fn changed_cells(previous: &LuaFrameState, target: &LuaFrameState) -> Vec<LuaCell> {
    let keys = previous
        .cells
        .keys()
        .chain(target.cells.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    keys.into_iter()
        .filter_map(|key| {
            let previous_cell = previous.cells.get(&key);
            let target_cell = target.cells.get(&key);
            if previous_cell == target_cell {
                return None;
            }
            let priority = target_cell
                .or(previous_cell)
                .map(|cell| cell.priority)
                .unwrap_or(LuaCellPriority::Normal);
            Some(LuaCell::new(
                key.region,
                key.index,
                target_cell.map(|cell| cell.glyph),
                priority,
            ))
        })
        .collect()
}

fn apply_cell(state: &mut LuaFrameState, cell: &LuaCell) {
    let key = CellKey {
        region: cell.region(),
        index: cell.index(),
    };
    if let Some(glyph) = cell.glyph() {
        state.cells.insert(
            key,
            StateCell {
                glyph,
                priority: cell.priority(),
            },
        );
    } else {
        state.cells.remove(&key);
    }
}
