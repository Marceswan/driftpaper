//! Immutable GPU programs shared by simulations on the same device.
use std::{collections::HashMap, sync::Mutex};

/// A device-scoped program cache. Share one `Arc<SharedResources>` across displays.
/// Simulation textures, uniforms and line state remain private to each `Flux`.
pub struct SharedResources {
    device: wgpu::Device,
    shaders: Mutex<HashMap<String, wgpu::ShaderModule>>,
    compute: Mutex<HashMap<String, wgpu::ComputePipeline>>,
    render: Mutex<HashMap<String, wgpu::RenderPipeline>>,
}

impl SharedResources {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            device: device.clone(),
            shaders: Mutex::new(HashMap::new()),
            compute: Mutex::new(HashMap::new()),
            render: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn belongs_to(&self, device: &wgpu::Device) -> bool {
        self.device == *device
    }

    pub(crate) fn create_shader_module(
        &self,
        descriptor: wgpu::ShaderModuleDescriptor<'_>,
    ) -> wgpu::ShaderModule {
        // Only internal, fixed shaders use this cache; labels are unique program IDs.
        let key = descriptor
            .label
            .expect("cached shader needs a unique label")
            .to_owned();
        self.shaders
            .lock()
            .unwrap()
            .entry(key)
            .or_insert_with(|| self.device.create_shader_module(descriptor))
            .clone()
    }

    pub(crate) fn create_compute_pipeline(
        &self,
        descriptor: &wgpu::ComputePipelineDescriptor<'_>,
    ) -> wgpu::ComputePipeline {
        let key = descriptor
            .label
            .expect("cached pipeline needs a unique label")
            .to_owned();
        self.compute
            .lock()
            .unwrap()
            .entry(key)
            .or_insert_with(|| self.device.create_compute_pipeline(descriptor))
            .clone()
    }

    pub(crate) fn create_render_pipeline(
        &self,
        descriptor: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> wgpu::RenderPipeline {
        let key = format!(
            "{}:{:?}",
            descriptor
                .label
                .expect("cached pipeline needs a unique label"),
            descriptor.fragment.as_ref().map(|f| f.targets)
        );
        self.render
            .lock()
            .unwrap()
            .entry(key)
            .or_insert_with(|| self.device.create_render_pipeline(descriptor))
            .clone()
    }

    #[cfg(test)]
    pub(crate) fn program_counts(&self) -> (usize, usize, usize) {
        (
            self.shaders.lock().unwrap().len(),
            self.compute.lock().unwrap().len(),
            self.render.lock().unwrap().len(),
        )
    }
}
