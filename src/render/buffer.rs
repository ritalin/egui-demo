use std::num::NonZero;


pub fn make_index_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Foreground index buffer"),
        size,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub fn make_vertex_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Foreground vertex buffer"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub fn measure_buffer_size(triangles: &[egui::ClippedPrimitive]) -> (u64, u64) {
    let (vertex_count, index_count) = triangles.iter()
        .fold((0, 0), |(vcount, icount), p| match p {
            egui::ClippedPrimitive{ primitive: egui::epaint::Primitive::Mesh(egui::Mesh{ indices, vertices, .. }), .. } => {
                (vcount + vertices.len(), icount + indices.len())
            }
            egui::ClippedPrimitive{ primitive: egui::epaint::Primitive::Callback(_), .. } => {
                (vcount, icount)
            }
        })
    ;
    ((vertex_count * size_of::<egui::epaint::Vertex>()) as u64, (index_count * size_of::<u32>()) as u64)
}

pub fn update_vertex_buffer(device: &mut wgpu::Device, queue: &wgpu::Queue, buffer_size: u64, triangles: &[egui::ClippedPrimitive], buffer: &mut wgpu::Buffer) {
    if buffer.size() <= buffer_size {
        *buffer = make_vertex_buffer(device, buffer_size * 2);
    }
    let Some(mut view) = queue.write_buffer_with(buffer, 0, NonZero::<u64>::new(buffer.size()).unwrap())
        else { unreachable!("Unexpected vertex buffer error") }
    ;
    let mut offset = 0;
    for egui::ClippedPrimitive{ primitive, .. } in triangles.iter() {
        match primitive {
            egui::epaint::Primitive::Mesh(egui::Mesh{ vertices, .. }) => {
                let start = offset;
                let end = offset + vertices.len() * size_of::<egui::epaint::Vertex>();
                offset = end;
                view[start..end].copy_from_slice(bytemuck::cast_slice(vertices));
            }
            egui::epaint::Primitive::Callback(_) => {}
        }
    }
}

pub fn update_index_buffer(device: &mut wgpu::Device, queue: &wgpu::Queue, buffer_size: u64, triangles: &[egui::ClippedPrimitive], buffer: &mut wgpu::Buffer) {
    if buffer.size() <= buffer_size {
        *buffer = make_index_buffer(device, buffer_size * 2);
    }
    let Some(mut view) = queue.write_buffer_with(buffer, 0, NonZero::<u64>::new(buffer.size() as u64).unwrap())
        else { unreachable!("Unexpected index buffer error") }
    ;

    let mut offset = 0;
    for egui::ClippedPrimitive{ primitive, .. } in triangles.iter() {
        match primitive {
            egui::epaint::Primitive::Mesh(egui::Mesh{ indices, .. }) => {
                let start = offset;
                let end = offset + indices.len() * size_of::<u32>();
                offset = end;
                view[start..end].copy_from_slice(bytemuck::cast_slice(indices));
            }
            egui::epaint::Primitive::Callback(_) => {}
        }
    }

}
