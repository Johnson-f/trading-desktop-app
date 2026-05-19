use std::collections::HashMap;

use crate::color::Rgba;

#[derive(Clone, Copy, Debug)]
pub enum ParamValue {
    Int(i32),
    Float(f32),
    Color(Rgba),
}

#[derive(Clone, Copy, Debug)]
pub enum ParamKind {
    Int { default: i32, min: i32, max: i32 },
    Float { default: f32, min: f32, max: f32 },
    Color { default: Rgba },
}

#[derive(Clone, Copy, Debug)]
pub struct ParamField {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
}

#[derive(Clone, Copy, Debug)]
pub struct ParamSchema {
    pub fields: &'static [ParamField],
}

#[derive(Clone, Debug)]
pub struct ParamValues(pub HashMap<&'static str, ParamValue>);

impl ParamSchema {
    pub fn defaults(&self) -> ParamValues {
        let mut map = HashMap::new();
        for f in self.fields {
            let v = match f.kind {
                ParamKind::Int { default, .. } => ParamValue::Int(default),
                ParamKind::Float { default, .. } => ParamValue::Float(default),
                ParamKind::Color { default } => ParamValue::Color(default),
            };
            map.insert(f.key, v);
        }
        ParamValues(map)
    }
}

impl ParamValues {
    pub fn int(&self, key: &str) -> i32 {
        match self.0.get(key) {
            Some(ParamValue::Int(v)) => *v,
            _ => panic!("param {key} missing or not Int"),
        }
    }
    pub fn float(&self, key: &str) -> f32 {
        match self.0.get(key) {
            Some(ParamValue::Float(v)) => *v,
            _ => panic!("param {key} missing or not Float"),
        }
    }
    pub fn color(&self, key: &str) -> Rgba {
        match self.0.get(key) {
            Some(ParamValue::Color(v)) => *v,
            _ => panic!("param {key} missing or not Color"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schema() -> ParamSchema {
        static FIELDS: &[ParamField] = &[
            ParamField {
                key: "period",
                label: "Period",
                kind: ParamKind::Int {
                    default: 14,
                    min: 1,
                    max: 500,
                },
            },
            ParamField {
                key: "mult",
                label: "Multiplier",
                kind: ParamKind::Float {
                    default: 2.0,
                    min: 0.1,
                    max: 10.0,
                },
            },
            ParamField {
                key: "color",
                label: "Color",
                kind: ParamKind::Color {
                    default: Rgba::from_rgb(10, 20, 30),
                },
            },
        ];
        ParamSchema { fields: FIELDS }
    }

    #[test]
    fn defaults_populate_all_fields() {
        let v = sample_schema().defaults();
        assert_eq!(v.int("period"), 14);
        assert!((v.float("mult") - 2.0).abs() < f32::EPSILON);
        assert_eq!(v.color("color"), Rgba::from_rgb(10, 20, 30));
    }

    #[test]
    #[should_panic(expected = "missing or not Int")]
    fn accessing_missing_param_panics() {
        let v = sample_schema().defaults();
        let _ = v.int("missing");
    }
}
