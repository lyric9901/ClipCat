struct Params {
    softness: f32,
    invert: f32,
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let from_col = from_at(uv);
    let to_col = to_at(uv);
    var luma = luminance(from_col.rgb);
    if (params.invert > 0.5) {
        luma = 1.0 - luma;
    }
    let soft = max(params.softness * 0.2, 0.001);
    let edge = smoothstep(progress - soft, progress + soft, luma);
    return mix(to_col, from_col, edge);
}
