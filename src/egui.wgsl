
struct VertexInputStub {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: u32,
};

struct VertexOutputStub {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// [u8; 4] SRGB as u32 -> [r, g, b, a] in 0.-1
fn unpack_color(color: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(color & 255u),
        f32((color >> 8u) & 255u),
        f32((color >> 16u) & 255u),
        f32((color >> 24u) & 255u),
    ) / 255.0;
}

@vertex
fn vs_main(model: VertexInputStub) -> VertexOutputStub {
    var out: VertexOutputStub;

    // --- 座標変換のスタブ化 (800x600固定と仮定) ---
    // eguiの(0,0)〜(1360.0, 768.0)を、wgpuの(-1,-1)〜(1,1)へ変換する
    let screen_size = vec2<f32>(1360.0, 768.0);
    let x = (model.position.x / screen_size.x) * 2.0 - 1.0;
    let y = 1.0 - (model.position.y / screen_size.y) * 2.0; // Y軸反転

    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.color = unpack_color(model.color); // そのままフラグメントシェーダーへ渡す
    return out;
}

@fragment
fn fs_main(in: VertexOutputStub) -> @location(0) vec4<f32> {
    return in.color;
}
