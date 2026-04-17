use zaned_chart_core::{CandleData, ComputedSeries, InputSpec, ParamValues};

use super::registry;
use super::trait_def::{Indicator, RenderTarget};

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
    next_id: u64,
}

impl Default for IndicatorManager {
    fn default() -> Self {
        Self {
            active: Vec::new(),
            next_id: 1,
        }
    }
}

impl IndicatorManager {
    pub fn add(&mut self, def_id: &'static str, params: ParamValues) -> Option<u64> {
        let def = registry::get(def_id)?;
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

    pub fn update_params(&mut self, instance_id: u64, params: ParamValues) {
        if let Some(a) = self.active.iter_mut().find(|a| a.instance_id == instance_id) {
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
    use super::*;
    use super::super::kinds::ema;
    use super::super::params::{ParamValue, ParamValues};
    use super::super::super::candle::{CandleData, CandleInstance};
    use std::collections::HashMap;

    fn sample_data(n: usize) -> CandleData {
        let instances: Vec<CandleInstance> = (0..n)
            .map(|i| CandleInstance {
                index: i as f32, open: 1.0, high: 1.0, low: 1.0,
                close: (i + 1) as f32, volume: 100.0,
            })
            .collect();
        CandleData { instances, dates: vec![String::new(); n] }
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
        new_params.insert("color", ParamValue::Color(zaned_chart_core::Rgba::from_rgb(255, 0, 0)));
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
