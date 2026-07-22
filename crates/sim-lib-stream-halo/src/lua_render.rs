//! Open Lua cell primitives for the Halo display link.

/// Logical `scene/glance` region addressed by a Lua cell update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LuaRegion {
    /// Card title glyphs.
    Title,
    /// Metric label glyphs.
    MetricLabel,
    /// Metric value glyphs.
    MetricValue,
    /// Action label glyphs.
    ActionLabel,
    /// Urgency token glyphs.
    Urgency,
    /// Safety-warrant marker.
    Warrant,
}

impl LuaRegion {
    /// Stable Lua protocol token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::MetricLabel => "metric_label",
            Self::MetricValue => "metric_value",
            Self::ActionLabel => "action_label",
            Self::Urgency => "urgency",
            Self::Warrant => "warrant",
        }
    }
}

/// Scheduling priority for one changed Lua cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LuaCellPriority {
    /// Safety-warrant content that must be attempted first.
    Warrant,
    /// Urgency-state content.
    Urgent,
    /// Ordinary glance content.
    Normal,
}

/// One changed glyph or cleared cell in the open Lua display protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LuaCell {
    region: LuaRegion,
    index: u16,
    glyph: Option<char>,
    priority: LuaCellPriority,
}

impl LuaCell {
    /// Builds a changed Lua cell.
    pub fn new(
        region: LuaRegion,
        index: u16,
        glyph: Option<char>,
        priority: LuaCellPriority,
    ) -> Self {
        Self {
            region,
            index,
            glyph,
            priority,
        }
    }

    /// Logical region containing this cell.
    pub fn region(&self) -> LuaRegion {
        self.region
    }

    /// Glyph offset within the region.
    pub fn index(&self) -> u16 {
        self.index
    }

    /// Replacement glyph, or `None` to clear the cell.
    pub fn glyph(&self) -> Option<char> {
        self.glyph
    }

    /// Scheduling priority.
    pub fn priority(&self) -> LuaCellPriority {
        self.priority
    }

    /// Exact encoded byte length on the Lua protocol link.
    pub fn byte_len(&self) -> u32 {
        u32::try_from(self.to_lua_primitive().len()).unwrap_or(u32::MAX)
    }

    /// Encodes this cell as one injection-safe Lua primitive.
    pub fn to_lua_primitive(&self) -> Vec<u8> {
        let glyph = self
            .glyph
            .map(|glyph| u32::from(glyph).to_string())
            .unwrap_or_else(|| "nil".to_owned());
        format!(
            "sim.set_cell(\"{}\",{},{glyph})\n",
            self.region.token(),
            self.index
        )
        .into_bytes()
    }
}

/// Encodes changed cells as an ordered Lua primitive frame.
pub fn encode_lua_cells(cells: &[LuaCell]) -> Vec<u8> {
    let capacity = cells
        .iter()
        .map(|cell| usize::try_from(cell.byte_len()).unwrap_or(usize::MAX))
        .fold(0usize, usize::saturating_add);
    let mut bytes = Vec::with_capacity(capacity);
    for cell in cells {
        bytes.extend(cell.to_lua_primitive());
    }
    bytes
}
