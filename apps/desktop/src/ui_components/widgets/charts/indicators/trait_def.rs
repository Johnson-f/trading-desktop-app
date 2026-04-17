use egui::{Painter, Rect};

use zaned_chart_core::{CandleData, ComputedSeries, InputSpec, LegendEntry, ParamValues};

use super::super::camera::Camera;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderTarget {
    MainOverlay,
    SubPane,
}

pub trait Indicator: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self, params: &ParamValues) -> String;
    fn target(&self) -> RenderTarget;

    /// Declare which series this indicator consumes. Order matters: `compute`
    /// receives them as `&[&[f32]]` in the same order.
    fn inputs(&self, params: &ParamValues) -> Vec<InputSpec>;

    /// Pure computation. Takes pre-resolved input series; doesn't touch CandleData.
    fn compute(&self, inputs: &[&[f32]], params: &ParamValues) -> ComputedSeries;

    #[allow(unused_variables)]
    fn draw_main(
        &self,
        painter: &Painter,
        rect: Rect,
        camera: &Camera,
        data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
    ) {
    }

    #[allow(unused_variables)]
    fn draw_pane(
        &self,
        painter: &Painter,
        pane_rect: Rect,
        x_mapper: &dyn Fn(f32) -> f32,
        data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
    ) {
    }

    #[allow(unused_variables)]
    fn legend(
        &self,
        data: &CandleData,
        computed: &ComputedSeries,
        params: &ParamValues,
        cursor_idx: Option<usize>,
    ) -> Vec<LegendEntry> {
        Vec::new()
    }
}
