pub(crate) fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = match pollster::block_on(
        instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
    ) {
        Ok(adapter) => adapter,
        Err(error) => {
            assert!(
                std::env::var_os("DRIFTPAPER_REQUIRE_GPU_TESTS").is_none(),
                "GPU tests required, but no adapter: {error}"
            );
            eprintln!("GPU smoke test skipped: {error}. Set DRIFTPAPER_REQUIRE_GPU_TESTS=1 to require an adapter.");
            return None;
        }
    };
    Some(
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("flux GPU regression tests"),
            required_features: wgpu::Features::FLOAT32_FILTERABLE,
            ..Default::default()
        }))
        .expect("request regression GPU device"),
    )
}

pub(crate) fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    bytes_per_pixel: u32,
) -> Vec<u8> {
    let size = texture.size();
    let row_bytes = size.width * bytes_per_pixel;
    let padded_row_bytes = row_bytes.div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GPU regression readback"),
        size: padded_row_bytes as u64 * size.height as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let (send, recv) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap()
        });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    recv.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let mut output = Vec::with_capacity(row_bytes as usize * size.height as usize);
    for row in mapped.chunks(padded_row_bytes as usize) {
        output.extend_from_slice(&row[..row_bytes as usize]);
    }
    drop(mapped);
    buffer.unmap();
    output
}
