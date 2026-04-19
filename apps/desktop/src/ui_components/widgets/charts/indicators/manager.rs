use std::collections::{HashMap, VecDeque};

use zaned_chart_core::{CandleData, ComputedSeries, InputSpec, ParamValues};

use super::registry;
use super::trait_def::{Indicator, RenderTarget};

const MAX_RECENTS: usize = 7;

pub struct ActiveIndicator {
    pub instance_id: u64,
    pub def_id: &'static str,
    pub params: ParamValues,
    pub(super) indicator: Box<dyn Indicator>,
    pub(super) cache: Option<ComputedSeries>,
}

impl ActiveIndicator {
    pub fn indicator(&self) -> &dyn Indicator {
        self.indicator.as_ref()
    }
    pub fn cached(&self) -> Option<&ComputedSeries> {
        self.cache.as_ref()
    }
    pub fn target(&self) -> RenderTarget {
        self.indicator.target()
    }
}

pub struct IndicatorManager {
    pub active: Vec<ActiveIndicator>,
    /// Most-recently-interacted-with def_ids, front-first. Populated by
    /// `add`; used by the shortcut toolbar. In-memory only.
    pub recents: VecDeque<&'static str>,
    /// Params of the most recent family cleared via `remove_all_of`, keyed
    /// by def_id. A subsequent `restore_or_add_default` for that def_id
    /// replays this set; any direct `add` discards it.
    stashed: HashMap<&'static str, Vec<ParamValues>>,
    next_id: u64,
}

impl Default for IndicatorManager {
    fn default() -> Self {
        Self {
            active: Vec::new(),
            recents: VecDeque::new(),
            stashed: HashMap::new(),
            next_id: 1,
        }
    }
}

impl IndicatorManager {
    pub fn add(&mut self, def_id: &'static str, params: ParamValues) -> Option<u64> {
        let def = registry::get(def_id)?;
        self.push_recent(def.id);
        // An explicit add supersedes any stashed set for this family — the
        // user has chosen a new configuration, so the old one is forgotten.
        self.stashed.remove(def.id);
        let id = self.next_id;
        self.next_id += 1;
        self.active.push(ActiveIndicator {
            instance_id: id,
            def_id: def.id,
            params,
            indicator: (def.factory)(),
            cache: None,
        });
        Some(id)
    }

    pub fn remove(&mut self, instance_id: u64) {
        self.active.retain(|a| a.instance_id != instance_id);
    }

    /// Remove every active instance of the given def_id, stashing the cleared
    /// params so the next `restore_or_add_default` for this family replays
    /// them instead of just adding a single default.
    pub fn remove_all_of(&mut self, def_id: &str) {
        let first_match = self.active.iter().find(|a| a.def_id == def_id);
        let Some(static_id) = first_match.map(|a| a.def_id) else {
            return;
        };
        let params: Vec<ParamValues> = self
            .active
            .iter()
            .filter(|a| a.def_id == def_id)
            .map(|a| a.params.clone())
            .collect();
        self.stashed.insert(static_id, params);
        self.active.retain(|a| a.def_id != def_id);
    }

    /// Toggle-on entry point for shortcut buttons. Replays the most recently
    /// stashed set of instances if one exists; otherwise adds a single
    /// instance with default params.
    pub fn restore_or_add_default(&mut self, def_id: &'static str) {
        if let Some(stashed) = self.stashed.remove(def_id) {
            for params in stashed {
                self.add(def_id, params);
            }
        } else if let Some(def) = registry::get(def_id) {
            self.add(def_id, def.params.defaults());
        }
    }

    pub fn has_any_of(&self, def_id: &str) -> bool {
        self.active.iter().any(|a| a.def_id == def_id)
    }

    fn push_recent(&mut self, def_id: &'static str) {
        self.recents.retain(|id| *id != def_id);
        self.recents.push_front(def_id);
        while self.recents.len() > MAX_RECENTS {
            self.recents.pop_back();
        }
    }

    pub fn update_params(&mut self, instance_id: u64, params: ParamValues) {
        if let Some(a) = self
            .active
            .iter_mut()
            .find(|a| a.instance_id == instance_id)
        {
            a.params = params;
            a.cache = None;
        }
    }

    pub fn ensure_computed(&mut self, data: &CandleData) {
        for a in &mut self.active {
            if a.cache.is_some() {
                continue;
            }
            let specs = a.indicator.inputs(&a.params);
            let resolved: Vec<Vec<f32>> = specs
                .iter()
                .map(|spec| resolve_input(*spec, data))
                .collect();
            let refs: Vec<&[f32]> = resolved.iter().map(|v| v.as_slice()).collect();
            a.cache = Some(a.indicator.compute(&refs, &a.params));
        }
    }

    /// Drop every cached series. Callers invoke this when the underlying
    /// `CandleData` is swapped (e.g. bucket size or timeframe change) so the
    /// next `ensure_computed` recomputes against the new series.
    pub fn invalidate_cache(&mut self) {
        for a in &mut self.active {
            a.cache = None;
        }
    }

    pub fn main_overlays(&self) -> impl Iterator<Item = (&ActiveIndicator, &ComputedSeries)> {
        self.active
            .iter()
            .filter(|a| a.indicator.target() == RenderTarget::MainOverlay)
            .filter_map(|a| a.cache.as_ref().map(|c| (a, c)))
    }

    pub fn sub_panes(&self) -> impl Iterator<Item = (&ActiveIndicator, &ComputedSeries)> {
        self.active
            .iter()
            .filter(|a| a.indicator.target() == RenderTarget::SubPane)
            .filter_map(|a| a.cache.as_ref().map(|c| (a, c)))
    }

    pub fn sub_pane_count(&self) -> usize {
        self.active
            .iter()
            .filter(|a| a.indicator.target() == RenderTarget::SubPane)
            .count()
    }
}

fn resolve_input(spec: InputSpec, data: &CandleData) -> Vec<f32> {
    match spec {
        InputSpec::Closes => data.instances.iter().map(|c| c.close).collect(),
        InputSpec::Opens => data.instances.iter().map(|c| c.open).collect(),
        InputSpec::Highs => data.instances.iter().map(|c| c.high).collect(),
        InputSpec::Lows => data.instances.iter().map(|c| c.low).collect(),
        InputSpec::Volumes => data.instances.iter().map(|c| c.volume).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::candle::{CandleData, CandleInstance};
    use super::super::kinds::ema;
    use super::super::params::{ParamValue, ParamValues};
    use super::*;
    use std::collections::HashMap;

    fn sample_data(n: usize) -> CandleData {
        let instances: Vec<CandleInstance> = (0..n)
            .map(|i| CandleInstance {
                index: i as f32,
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: (i + 1) as f32,
                volume: 100.0,
            })
            .collect();
        CandleData {
            instances,
            dates: vec![String::new(); n],
        }
    }

    #[test]
    fn add_returns_increasing_ids() {
        let mut m = IndicatorManager::default();
        let a = m.add(ema::ID, ema::SCHEMA.defaults()).unwrap();
        let b = m.add(ema::ID, ema::SCHEMA.defaults()).unwrap();
        assert_ne!(a, b);
        assert_eq!(m.active.len(), 2);
    }

    #[test]
    fn add_unknown_def_returns_none() {
        let mut m = IndicatorManager::default();
        assert!(m.add("nope", ema::SCHEMA.defaults()).is_none());
        assert_eq!(m.active.len(), 0);
    }

    #[test]
    fn remove_drops_instance() {
        let mut m = IndicatorManager::default();
        let a = m.add(ema::ID, ema::SCHEMA.defaults()).unwrap();
        m.remove(a);
        assert_eq!(m.active.len(), 0);
    }

    #[test]
    fn ensure_computed_populates_cache() {
        let mut m = IndicatorManager::default();
        m.add(ema::ID, ema::SCHEMA.defaults()).unwrap();
        let data = sample_data(30);
        assert!(m.active[0].cache.is_none());
        m.ensure_computed(&data);
        assert!(m.active[0].cache.is_some());
    }

    #[test]
    fn ensure_computed_is_idempotent_when_params_unchanged() {
        let mut m = IndicatorManager::default();
        m.add(ema::ID, ema::SCHEMA.defaults()).unwrap();
        let data = sample_data(30);
        m.ensure_computed(&data);
        let first = m.active[0].cache.as_ref().unwrap() as *const _;
        m.ensure_computed(&data);
        let second = m.active[0].cache.as_ref().unwrap() as *const _;
        assert_eq!(first, second, "cache should not be regenerated");
    }

    #[test]
    fn update_params_invalidates_cache() {
        let mut m = IndicatorManager::default();
        let id = m.add(ema::ID, ema::SCHEMA.defaults()).unwrap();
        let data = sample_data(30);
        m.ensure_computed(&data);
        assert!(m.active[0].cache.is_some());
        let mut new_params = HashMap::new();
        new_params.insert("period", ParamValue::Int(10));
        new_params.insert(
            "color",
            ParamValue::Color(zaned_chart_core::Rgba::from_rgb(255, 0, 0)),
        );
        m.update_params(id, ParamValues(new_params));
        assert!(m.active[0].cache.is_none());
    }

    #[test]
    fn sub_pane_count_ignores_main_overlays() {
        let mut m = IndicatorManager::default();
        m.add(ema::ID, ema::SCHEMA.defaults()).unwrap();
        assert_eq!(m.sub_pane_count(), 0);
    }
}
