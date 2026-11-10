//! Checks that drivers with unsafe concurrent compiles reach their compiler only under the
//! per-device compilation guard.
//!
//! The device is opened on whichever Vulkan adapter is present (lavapipe in CI) with the
//! workaround forced on, and its shader, pipeline and pipeline-cache entry points are wrapped so
//! each call records whether the guard was held.

use alloc::{
    borrow::Cow,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::ffi::{c_void, CStr};
use std::sync::{Mutex, OnceLock};

use ash::vk;

use crate::{Device as _, Instance as _};

/// Every call observed through the wrapped entry points, with whether the guard was held.
static CALLS: Mutex<Vec<(&'static str, bool)>> = Mutex::new(Vec::new());
/// The driver's own entry points that the wrappers forward to.
static DRIVER: OnceLock<ash::DeviceFnV1_0> = OnceLock::new();
/// The device whose guard the wrappers inspect.
static DEVICE: OnceLock<Weak<super::DeviceShared>> = OnceLock::new();

/// Records `name` and whether the device's compilation guard is currently held.
fn observe(name: &'static str) {
    let held = DEVICE
        .get()
        .and_then(Weak::upgrade)
        .is_some_and(|shared| shared.shader_compilation.is_locked());
    CALLS.lock().unwrap().push((name, held));
}

fn driver() -> &'static ash::DeviceFnV1_0 {
    DRIVER.get().expect("driver entry points are loaded")
}

unsafe extern "system" fn create_shader_module(
    device: vk::Device,
    info: *const vk::ShaderModuleCreateInfo<'_>,
    allocator: *const vk::AllocationCallbacks<'_>,
    module: *mut vk::ShaderModule,
) -> vk::Result {
    observe("vkCreateShaderModule");
    unsafe { (driver().create_shader_module)(device, info, allocator, module) }
}

unsafe extern "system" fn destroy_shader_module(
    device: vk::Device,
    module: vk::ShaderModule,
    allocator: *const vk::AllocationCallbacks<'_>,
) {
    observe("vkDestroyShaderModule");
    unsafe { (driver().destroy_shader_module)(device, module, allocator) }
}

unsafe extern "system" fn create_graphics_pipelines(
    device: vk::Device,
    cache: vk::PipelineCache,
    count: u32,
    infos: *const vk::GraphicsPipelineCreateInfo<'_>,
    allocator: *const vk::AllocationCallbacks<'_>,
    pipelines: *mut vk::Pipeline,
) -> vk::Result {
    observe("vkCreateGraphicsPipelines");
    unsafe {
        (driver().create_graphics_pipelines)(device, cache, count, infos, allocator, pipelines)
    }
}

unsafe extern "system" fn create_compute_pipelines(
    device: vk::Device,
    cache: vk::PipelineCache,
    count: u32,
    infos: *const vk::ComputePipelineCreateInfo<'_>,
    allocator: *const vk::AllocationCallbacks<'_>,
    pipelines: *mut vk::Pipeline,
) -> vk::Result {
    observe("vkCreateComputePipelines");
    unsafe {
        (driver().create_compute_pipelines)(device, cache, count, infos, allocator, pipelines)
    }
}

unsafe extern "system" fn create_pipeline_cache(
    device: vk::Device,
    info: *const vk::PipelineCacheCreateInfo<'_>,
    allocator: *const vk::AllocationCallbacks<'_>,
    cache: *mut vk::PipelineCache,
) -> vk::Result {
    observe("vkCreatePipelineCache");
    unsafe { (driver().create_pipeline_cache)(device, info, allocator, cache) }
}

unsafe extern "system" fn get_pipeline_cache_data(
    device: vk::Device,
    cache: vk::PipelineCache,
    size: *mut usize,
    data: *mut c_void,
) -> vk::Result {
    observe("vkGetPipelineCacheData");
    unsafe { (driver().get_pipeline_cache_data)(device, cache, size, data) }
}

unsafe extern "system" fn destroy_pipeline_cache(
    device: vk::Device,
    cache: vk::PipelineCache,
    allocator: *const vk::AllocationCallbacks<'_>,
) {
    observe("vkDestroyPipelineCache");
    unsafe { (driver().destroy_pipeline_cache)(device, cache, allocator) }
}

/// Returns the wrapper for a compiler-facing entry point, or `None` to use the driver's own.
fn wrapper(name: &CStr) -> Option<*const c_void> {
    Some(match name.to_bytes() {
        b"vkCreateShaderModule" => create_shader_module as *const c_void,
        b"vkDestroyShaderModule" => destroy_shader_module as *const c_void,
        b"vkCreateGraphicsPipelines" => create_graphics_pipelines as *const c_void,
        b"vkCreateComputePipelines" => create_compute_pipelines as *const c_void,
        b"vkCreatePipelineCache" => create_pipeline_cache as *const c_void,
        b"vkGetPipelineCacheData" => get_pipeline_cache_data as *const c_void,
        b"vkDestroyPipelineCache" => destroy_pipeline_cache as *const c_void,
        _ => return None,
    })
}

/// Parses and validates `source` for a hal shader module.
fn naga_shader(source: &str) -> crate::NagaShader {
    let module = naga::front::wgsl::parse_str(source).expect("test shader parses");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("test shader validates");
    crate::NagaShader {
        module: Cow::Owned(module),
        info,
        debug_source: None,
    }
}

const SHADER: &str = "
@compute @workgroup_size(1)
fn main_cs() {}

@vertex
fn main_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32(index), 0.0, 0.0, 1.0);
}

@fragment
fn main_fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
";

#[test]
fn serialized_drivers_hold_the_compilation_guard_for_compiler_calls() {
    let instance = match unsafe {
        super::Instance::init(&crate::InstanceDescriptor {
            name: "compilation guard test",
            flags: wgt::InstanceFlags::empty(),
            memory_budget_thresholds: wgt::MemoryBudgetThresholds::default(),
            backend_options: wgt::BackendOptions::default(),
            telemetry: None,
            display: None,
        })
    } {
        Ok(instance) => instance,
        Err(error) => {
            std::eprintln!("missing fixture: Vulkan instance for the compilation guard ({error})");
            return;
        }
    };
    let Some(exposed) = unsafe { instance.enumerate_adapters(None) }
        .into_iter()
        .next()
    else {
        std::eprintln!("missing fixture: Vulkan adapter for the compilation guard");
        return;
    };
    let mut adapter = exposed.adapter;
    adapter.workarounds |= super::Workarounds::SERIALIZE_SHADER_COMPILATION;

    let features = wgt::Features::empty();
    let extensions = adapter.required_device_extensions(features);
    let mut device_features = adapter.physical_device_features(&extensions, features);
    let queue = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(0)
        .queue_priorities(&[1.0]);
    let extension_names = extensions
        .iter()
        .map(|name| name.as_ptr())
        .collect::<Vec<_>>();
    let info = device_features.add_to_device_create(
        vk::DeviceCreateInfo::default()
            .queue_create_infos(core::slice::from_ref(&queue))
            .enabled_extension_names(&extension_names),
    );
    let raw = unsafe { adapter.instance.raw.create_device(adapter.raw, &info, None) }
        .expect("test device opens");
    let load = |name: &CStr| unsafe {
        core::mem::transmute::<vk::PFN_vkVoidFunction, *const c_void>(
            adapter
                .instance
                .raw
                .get_device_proc_addr(raw.handle(), name.as_ptr()),
        )
    };
    let _ = DRIVER.set(ash::DeviceFnV1_0::load(load));
    let wrapped = unsafe {
        ash::Device::load_with(
            |name| wrapper(name).unwrap_or_else(|| load(name)),
            raw.handle(),
        )
    };
    let open = unsafe {
        adapter.device_from_raw(
            wrapped,
            None,
            &extensions,
            features,
            &exposed.capabilities.limits,
            &wgt::MemoryHints::default(),
            0,
            0,
        )
    }
    .expect("test device wraps");
    let device = open.device;
    let _ = DEVICE.set(Arc::downgrade(&device.shared));

    unsafe {
        let layout = device
            .create_pipeline_layout(&crate::PipelineLayoutDescriptor {
                label: None,
                flags: crate::PipelineLayoutFlags::empty(),
                bind_group_layouts: &[],
                immediate_size: 0,
            })
            .unwrap();
        let cache = device
            .create_pipeline_cache(&crate::PipelineCacheDescriptor {
                label: None,
                data: None,
            })
            .unwrap();
        let shader = naga_shader(SHADER);
        let spirv = naga::back::spv::write_vec(
            &shader.module,
            &shader.info,
            &naga::back::spv::Options::default(),
            None,
        )
        .unwrap();
        let descriptor = crate::ShaderModuleDescriptor {
            label: None,
            runtime_checks: wgt::ShaderRuntimeChecks::checked(),
        };
        let module = device
            .create_shader_module(&descriptor, crate::ShaderInput::Naga(shader))
            .unwrap();
        let constants = Default::default();
        let stage = |entry_point| crate::ProgrammableStage {
            module: &module,
            entry_point,
            constants: &constants,
            zero_initialize_workgroup_memory: true,
        };
        let compute = device
            .create_compute_pipeline(&crate::ComputePipelineDescriptor {
                label: None,
                layout: &layout,
                stage: stage("main_cs"),
                cache: Some(&cache),
            })
            .unwrap();
        let render = device
            .create_render_pipeline(&crate::RenderPipelineDescriptor {
                label: None,
                layout: &layout,
                vertex_processor: crate::VertexProcessor::Standard {
                    vertex_buffers: &[],
                    vertex_stage: stage("main_vs"),
                },
                primitive: wgt::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgt::MultisampleState::default(),
                fragment_stage: Some(stage("main_fs")),
                color_targets: &[Some(wgt::ColorTargetState {
                    format: wgt::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgt::ColorWrites::ALL,
                })],
                multiview_mask: None,
                cache: Some(&cache),
            })
            .unwrap();
        let raw_module = device
            .create_shader_module(&descriptor, crate::ShaderInput::SpirV(&spirv))
            .unwrap();
        let _ = device.pipeline_cache_get_data(&cache);

        device.destroy_shader_module(raw_module);
        device.destroy_render_pipeline(render);
        device.destroy_compute_pipeline(compute);
        device.destroy_pipeline_cache(cache);
        device.destroy_shader_module(module);
        device.destroy_pipeline_layout(layout);
    }
    drop(open.queue);
    drop(device);

    let calls = core::mem::take(&mut *CALLS.lock().unwrap());
    for name in [
        "vkCreateShaderModule",
        "vkDestroyShaderModule",
        "vkCreateGraphicsPipelines",
        "vkCreateComputePipelines",
        "vkCreatePipelineCache",
        "vkGetPipelineCacheData",
        "vkDestroyPipelineCache",
    ] {
        assert!(
            calls.iter().any(|&(called, _)| called == name),
            "{name} was not exercised: {calls:?}"
        );
    }
    let unguarded = calls
        .iter()
        .filter(|&&(_, held)| !held)
        .map(|&(name, _)| name)
        .collect::<Vec<_>>();
    assert!(
        unguarded.is_empty(),
        "called without the guard: {unguarded:?}"
    );
}
