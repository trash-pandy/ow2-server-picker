use std::cmp::Ordering;

use crate::{ping, prefixes};

pub struct RegionEntry {
    pub region: prefixes::Region,
    pub ping: ping::PingStatus,
    pub selected: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegionSortBy {
    Name,
    Ping,
}

impl std::fmt::Display for RegionSortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            RegionSortBy::Name => "name",
            RegionSortBy::Ping => "ping",
        };
        write!(f, "{name}")
    }
}

pub struct RegionSorting {
    pub by: RegionSortBy,
    pub asc: bool,
}

impl RegionSorting {
    pub fn ordering_name(&self) -> &'static str {
        if self.asc { "ascending" } else { "descending" }
    }

    pub fn toggle_asc(&mut self) {
        self.asc = !self.asc
    }

    pub fn next_property(&self) -> RegionSortBy {
        match self.by {
            RegionSortBy::Name => RegionSortBy::Ping,
            RegionSortBy::Ping => RegionSortBy::Name,
        }
    }

    pub fn cycle_property(&mut self) {
        self.by = self.next_property()
    }

    pub fn as_cmp(&self) -> impl Fn(&RegionEntry, &RegionEntry) -> Ordering {
        let by = self.by;
        let asc = self.asc;

        move |a, b| {
            let ord = match by {
                RegionSortBy::Name => a.region.name.cmp(&b.region.name),
                RegionSortBy::Ping => {
                    let a_ping = a.ping.as_millis_or(1000);
                    let b_ping = b.ping.as_millis_or(1000);
                    a_ping.cmp(&b_ping)
                }
            };

            if asc { ord } else { ord.reverse() }
        }
    }
}

impl Default for RegionSorting {
    fn default() -> Self {
        Self {
            by: RegionSortBy::Name,
            asc: true,
        }
    }
}
