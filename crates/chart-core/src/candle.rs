use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CandleInstance {
    pub index: f32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
}

#[derive(serde::Deserialize)]
pub struct JsonCandle {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

pub struct CandleData {
    pub instances: Vec<CandleInstance>,
    pub dates: Vec<String>,
}

impl CandleData {
    pub fn from_json(json_candles: &[JsonCandle]) -> Self {
        let instances: Vec<CandleInstance> = json_candles
            .iter()
            .enumerate()
            .map(|(i, c)| CandleInstance {
                index: i as f32,
                open: c.open as f32,
                high: c.high as f32,
                low: c.low as f32,
                close: c.close as f32,
                volume: c.volume as f32,
            })
            .collect();
        let dates = json_candles.iter().map(|c| c.date.clone()).collect();
        Self { instances, dates }
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}
