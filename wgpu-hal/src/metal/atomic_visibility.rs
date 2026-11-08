//! Makes texture atomics visible to later passes.
//!
//! On some Metal drivers, a pass that reads a texture after an earlier pass
//! changed it with texture atomics can read the value from before the atomics.
//! Apple's paravirtual GPU in macOS virtual machines does this when many
//! invocations add to the same texel. Metal's hazard tracking, an `MTLFence`
//! and a separate command buffer all leave the read stale. A compute pass in
//! between that binds the texture `read_write` makes it current.
//!
//! So when a pass that bound textures for atomics ends, the encoder adds one
//! compute pass that dispatches a single thread per such texture. Each thread
//! loads texel 0 and stores it back unchanged only if every bit is set, so the
//! texture's contents never change.

use alloc::{format, string::String, vec::Vec};
use core::ptr::NonNull;

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::{
    MTLCommandBuffer, MTLCommandEncoder, MTLComputeCommandEncoder, MTLComputePipelineState,
    MTLDevice, MTLLibrary, MTLPixelFormat, MTLSize, MTLTexture, MTLTextureType,
};

/// Texture types that storage bindings can use, with the MSL type name and
/// the coordinate arguments for texel 0.
const TEXTURE_TYPES: [(MTLTextureType, &str, &str); 5] = [
    (MTLTextureType::Type1D, "texture1d", "0u"),
    (MTLTextureType::Type1DArray, "texture1d_array", "0u, 0u"),
    (MTLTextureType::Type2D, "texture2d", "uint2(0u)"),
    (
        MTLTextureType::Type2DArray,
        "texture2d_array",
        "uint2(0u), 0u",
    ),
    (MTLTextureType::Type3D, "texture3d", "uint3(0u)"),
];

/// Sample types of atomic-capable pixel formats.
const SAMPLE_TYPES: [&str; 2] = ["uint", "int"];

/// The MSL sample type to use for an atomic texture with `format`, as an
/// index into [`SAMPLE_TYPES`].
fn sample_type(format: MTLPixelFormat) -> Option<usize> {
    match format {
        // `R64Uint` textures use `RG32Uint`.
        MTLPixelFormat::R32Uint | MTLPixelFormat::RG32Uint => Some(0),
        MTLPixelFormat::R32Sint => Some(1),
        _ => None,
    }
}

fn kernel_name(texture_type: &str, sample_type: &str) -> String {
    format!("touch_{texture_type}_{sample_type}")
}

fn source() -> String {
    let mut source = String::from("#include <metal_stdlib>\nusing namespace metal;\n");
    for (_, texture_type, coords) in TEXTURE_TYPES {
        for sample_type in SAMPLE_TYPES {
            let name = kernel_name(texture_type, sample_type);
            source += &format!(
                "kernel void {name}({texture_type}<{sample_type}, access::read_write> t [[texture(0)]]) {{\n\
                 \x20   {sample_type}4 v = t.read({coords});\n\
                 \x20   if (as_type<uint>(v.x) == 0xffffffffu) {{ t.write(v, {coords}); }}\n\
                 }}\n"
            );
        }
    }
    source
}

/// One pipeline per entry of [`TEXTURE_TYPES`] and [`SAMPLE_TYPES`].
#[derive(Debug)]
pub(super) struct TouchPipelines {
    pipelines: Vec<Retained<ProtocolObject<dyn MTLComputePipelineState>>>,
}

unsafe impl Send for TouchPipelines {}
unsafe impl Sync for TouchPipelines {}

impl TouchPipelines {
    /// Compiles the pipelines, or logs why it couldn't and returns `None`.
    pub(super) fn new(device: &ProtocolObject<dyn MTLDevice>) -> Option<Self> {
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(&source()), None)
            .inspect_err(|err| log::warn!("Metal atomic texture visibility pass: {err}"))
            .ok()?;
        let mut pipelines = Vec::with_capacity(TEXTURE_TYPES.len() * SAMPLE_TYPES.len());
        for (_, texture_type, _) in TEXTURE_TYPES {
            for sample_type in SAMPLE_TYPES {
                let function = library.newFunctionWithName(&NSString::from_str(&kernel_name(
                    texture_type,
                    sample_type,
                )))?;
                let pipeline = device
                    .newComputePipelineStateWithFunction_error(&function)
                    .inspect_err(|err| log::warn!("Metal atomic texture visibility pass: {err}"))
                    .ok()?;
                pipelines.push(pipeline);
            }
        }
        Some(Self { pipelines })
    }

    fn get(
        &self,
        texture: &ProtocolObject<dyn MTLTexture>,
    ) -> Option<&ProtocolObject<dyn MTLComputePipelineState>> {
        let texture_type = TEXTURE_TYPES
            .iter()
            .position(|&(ty, ..)| ty == texture.textureType())?;
        let sample_type = sample_type(texture.pixelFormat())?;
        Some(&self.pipelines[texture_type * SAMPLE_TYPES.len() + sample_type])
    }

    /// Encodes a compute pass into `command_buffer` that touches each of
    /// `textures` as the module documentation describes. Textures whose type
    /// or format has no pipeline are skipped.
    ///
    /// # Safety
    ///
    /// Each pointer in `textures` must point to a live texture.
    pub(super) unsafe fn encode(
        &self,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        textures: &[NonNull<ProtocolObject<dyn MTLTexture>>],
    ) {
        let Some(encoder) = command_buffer.computeCommandEncoder() else {
            return;
        };
        encoder.setLabel(Some(&NSString::from_str(
            "wgpu atomic texture visibility pass",
        )));
        let one = MTLSize {
            width: 1,
            height: 1,
            depth: 1,
        };
        for texture in textures {
            let texture = unsafe { texture.as_ref() };
            let Some(pipeline) = self.get(texture) else {
                continue;
            };
            encoder.setComputePipelineState(pipeline);
            unsafe { encoder.setTexture_atIndex(Some(texture), 0) };
            encoder.dispatchThreadgroups_threadsPerThreadgroup(one, one);
        }
        encoder.endEncoding();
    }
}
