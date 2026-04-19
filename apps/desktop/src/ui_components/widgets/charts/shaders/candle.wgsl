struct Camera {
    x_offset: f32,
    x_scale: f32,
    y_offset: f32,
    y_scale: f32,
    width: f32,
    height: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) candle_index: f32,
    @location(1) open: f32,
    @location(2) high: f32,
    @location(3) low: f32,
    @location(4) close: f32,
    @location(5) volume: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

fn to_clip(x_data: f32, y_data: f32) -> vec2<f32> {
    let x_pixel = (x_data - camera.x_offset) * camera.x_scale;
    let y_pixel = (y_data - camera.y_offset) * camera.y_scale;
    let x_clip = (x_pixel / camera.width) * 2.0 - 1.0;
    let y_clip = (y_pixel / camera.height) * 2.0 - 1.0;
    return vec2<f32>(x_clip, y_clip);
}

// Unit quad: 6 vertices = 2 triangles
const QUAD: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, 0.0), // bottom-left
    vec2<f32>( 1.0, 0.0), // bottom-right
    vec2<f32>( 1.0, 1.0), // top-right
    vec2<f32>(-1.0, 0.0), // bottom-left
    vec2<f32>( 1.0, 1.0), // top-right
    vec2<f32>(-1.0, 1.0), // top-left
);

// 18 vertices per candle: body (0-5), upper wick (6-11), lower wick (12-17)
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let is_up = in.close >= in.open;
    let body_top = max(in.open, in.close);
    let body_bottom = min(in.open, in.close);

    if is_up {
        out.color = vec3<f32>(0.149, 0.788, 0.627);
    } else {
        out.color = vec3<f32>(1.0, 0.624, 0.302);
    }

    let center_x = in.candle_index;
    let vi = in.vertex_index;
    var pos: vec2<f32>;

    // Wick: 1 pixel wide (0.5 px half-extent on each side).
    let wick_half = 0.5 / camera.x_scale;

    // Doji floor: body renders at least 1 px tall so open==close bars don't
    // vanish.
    let min_half_height = 0.5 / camera.y_scale;
    let center_y = (body_top + body_bottom) * 0.5;
    let half_h = max((body_top - body_bottom) * 0.5, min_half_height);
    let draw_bottom = center_y - half_h;
    let draw_top = center_y + half_h;

    if vi < 6u {
        // Body quad — 85% of the candle slot wide.
        let corner = QUAD[vi];
        let x_data = center_x + corner.x * 0.425;
        let y_data = draw_bottom + corner.y * (draw_top - draw_bottom);
        pos = to_clip(x_data, y_data);
    } else if vi < 12u {
        // Upper wick quad (body_top → high)
        let corner = QUAD[vi - 6u];
        let x_data = center_x + corner.x * wick_half;
        let y_data = body_top + corner.y * (in.high - body_top);
        pos = to_clip(x_data, y_data);
    } else {
        // Lower wick quad (low → body_bottom)
        let corner = QUAD[vi - 12u];
        let x_data = center_x + corner.x * wick_half;
        let y_data = in.low + corner.y * (body_bottom - in.low);
        pos = to_clip(x_data, y_data);
    }

    out.position = vec4<f32>(pos.x, pos.y, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
