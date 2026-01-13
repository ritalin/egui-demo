use std::{borrow::Cow, collections::hash_map};
use egui::ahash::HashMap;

pub fn into_sampler(device: &wgpu::Device, options: egui::TextureOptions, label: Option<&str>) -> wgpu::Sampler {
    let address_mode = match options.wrap_mode {
        egui::TextureWrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        egui::TextureWrapMode::Repeat => wgpu::AddressMode::Repeat,
        egui::TextureWrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
    };
    device.create_sampler(&wgpu::SamplerDescriptor {
        label,
        mag_filter: match options.magnification {
            egui::TextureFilter::Nearest => wgpu::FilterMode::Nearest,
            egui::TextureFilter::Linear => wgpu::FilterMode::Linear,
        },
        min_filter: match options.minification {
            egui::TextureFilter::Nearest => wgpu::FilterMode::Nearest,
            egui::TextureFilter::Linear => wgpu::FilterMode::Linear,
        },
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        ..Default::default()
    })
}

pub fn update_samplers(device: &wgpu::Device, texture_options: impl Iterator<Item = egui::TextureOptions>, samplers: &mut HashMap<egui::TextureOptions, wgpu::Sampler>) {
    for options in texture_options {
        if let hash_map::Entry::Vacant(entry) = samplers.entry(options) {
            entry.insert(into_sampler(device, options, None));
        }
    }
}

pub struct TextureResource {
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
}

pub fn send_texture_images_pos<'a>(
    queue: &wgpu::Queue,
    images: &[(egui::TextureId, egui::epaint::ImageDelta)],
    cache: &HashMap<egui::TextureId, TextureResource>)
{
    // send partially position
    for (id, img) in images.iter() {
        let (Some(pos), Some(res)) = (img.pos, cache.get(id)) else { continue };

        let data_bytes = match &img.image {
            egui::ImageData::Color(data) => Cow::Borrowed(&data.pixels),
        };
        let size = wgpu::Extent3d { width: img.image.width() as u32, height: img.image.height() as u32, depth_or_array_layers: 1 };
        send_texture_image_internal(queue, &res.texture, bytemuck::cast_slice(&data_bytes), wgpu::Origin3d { x: pos[0] as u32, y: pos[1] as u32, z: 0 }, size);
    }
}

pub fn into_texture(device: &wgpu::Device, size: wgpu::Extent3d, label: Option<&str>) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
    })
}

pub fn send_texture_images_new<'a>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    samplers: &'a HashMap<egui::TextureOptions, wgpu::Sampler>,
    images: &[(egui::TextureId, egui::epaint::ImageDelta)]) -> impl Iterator<Item = (egui::TextureId, wgpu::Texture, &'a wgpu::Sampler)>
{
    images.iter()
        .filter_map(|(id, img)| {
            if img.pos.is_some() { return None };

            // new texture
            let data_bytes = match &img.image {
                egui::ImageData::Color(data) => Cow::Borrowed(&data.pixels),
            };
            let size = wgpu::Extent3d { width: img.image.width() as u32, height: img.image.height() as u32, depth_or_array_layers: 1 };
            let texture = into_texture(device, size, Some(&format!("texture/id: {id:?}")));
            send_texture_image_internal(queue, &texture, bytemuck::cast_slice(&data_bytes), wgpu::Origin3d::ZERO, size);
            Some((*id, texture, samplers.get(&img.options).expect("Sampler must be configured")))
        })
}

fn send_texture_image_internal(queue: &wgpu::Queue, texture: &wgpu::Texture, data_bytes: &[u8], origin: wgpu::Origin3d, size: wgpu::Extent3d) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin,
            aspect: wgpu::TextureAspect::All,
        },
        data_bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * size.width),
            rows_per_image: Some(size.height),
        },
        size
    );
}

pub fn into_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture: &wgpu::Texture,
    sampler: &wgpu::Sampler,
    label: Option<&str>) -> wgpu::BindGroup
{
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label,
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&wgpu::TextureViewDescriptor::default())
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

pub fn update_bind_groups<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    textures: impl Iterator<Item = (egui::TextureId, wgpu::Texture, &'a wgpu::Sampler)>,
    cache: &mut HashMap<egui::TextureId, TextureResource>)
{
    for (id, texture, sampler) in textures {
        let bind_group = into_bind_group(device, layout, &texture, sampler, Some(&format!("bind-group/id: {id:?}")));
        cache.insert(id, TextureResource { texture, bind_group });
    }
}
