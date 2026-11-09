struct Params {
    red: f32,
    green: f32,
    blue: f32,
}

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let base = select(from_at(uv), to_at(uv), progress >= 0.5);
    let color_amount = 1.0 - abs(progress * 2.0 - 1.0);
    let target_color = vec3<f32>(params.red, params.green, params.blue);
    let rgb = from_display(mix(to_display(base.rgb), target_color, color_amount));
    return vec4<f32>(rgb, base.a);
}
