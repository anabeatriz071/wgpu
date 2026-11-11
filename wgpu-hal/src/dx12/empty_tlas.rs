use crate::auxil::dxgi::result::HResult as _;
use core::{ffi::c_void, mem::size_of, ptr};
use windows::{
    core::Interface as _,
    Win32::Graphics::{Direct3D12 as d3d, Dxgi::Common as dxgi},
};

/// A valid masked-out instance avoids WARP faults on empty and null-BLAS scenes.
pub(super) struct EmptyTlas {
    vertices: d3d::ID3D12Resource,
    blas: d3d::ID3D12Resource,
    scratch: d3d::ID3D12Resource,
    pub(super) instances: d3d::ID3D12Resource,
}

fn buffer(
    device: &d3d::ID3D12Device,
    size: u64,
    upload: bool,
    state: d3d::D3D12_RESOURCE_STATES,
) -> Result<d3d::ID3D12Resource, crate::DeviceError> {
    let mut resource = None;
    unsafe {
        device.CreateCommittedResource(
            &d3d::D3D12_HEAP_PROPERTIES {
                Type: if upload {
                    d3d::D3D12_HEAP_TYPE_UPLOAD
                } else {
                    d3d::D3D12_HEAP_TYPE_DEFAULT
                },
                CreationNodeMask: 1,
                VisibleNodeMask: 1,
                ..Default::default()
            },
            d3d::D3D12_HEAP_FLAG_NONE,
            &d3d::D3D12_RESOURCE_DESC {
                Dimension: d3d::D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: size,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                SampleDesc: dxgi::DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: d3d::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                Flags: if upload {
                    d3d::D3D12_RESOURCE_FLAG_NONE
                } else {
                    d3d::D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS
                },
                ..Default::default()
            },
            state,
            None,
            &mut resource,
        )
    }
    .into_device_result("Empty TLAS backing buffer")?;
    resource.ok_or(crate::DeviceError::Unexpected)
}

fn upload(
    device: &d3d::ID3D12Device,
    bytes: &[u8],
) -> Result<d3d::ID3D12Resource, crate::DeviceError> {
    let resource = buffer(
        device,
        bytes.len() as u64,
        true,
        d3d::D3D12_RESOURCE_STATE_GENERIC_READ,
    )?;
    let mut mapped = ptr::null_mut::<c_void>();
    unsafe { resource.Map(0, None, Some(&mut mapped)) }
        .into_device_result("Empty TLAS upload map")?;
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.cast(), bytes.len());
        resource.Unmap(0, None);
    }
    Ok(resource)
}

fn geometry(vertices: u64) -> d3d::D3D12_RAYTRACING_GEOMETRY_DESC {
    d3d::D3D12_RAYTRACING_GEOMETRY_DESC {
        Type: d3d::D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES,
        Flags: d3d::D3D12_RAYTRACING_GEOMETRY_FLAG_OPAQUE,
        Anonymous: d3d::D3D12_RAYTRACING_GEOMETRY_DESC_0 {
            Triangles: d3d::D3D12_RAYTRACING_GEOMETRY_TRIANGLES_DESC {
                VertexCount: 3,
                VertexFormat: dxgi::DXGI_FORMAT_R32G32B32_FLOAT,
                VertexBuffer: d3d::D3D12_GPU_VIRTUAL_ADDRESS_AND_STRIDE {
                    StartAddress: vertices,
                    StrideInBytes: size_of::<[f32; 3]>() as u64,
                },
                ..Default::default()
            },
        },
    }
}

fn inputs(
    geometry: &d3d::D3D12_RAYTRACING_GEOMETRY_DESC,
) -> d3d::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
    d3d::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
        Type: d3d::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL,
        Flags: d3d::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_TRACE,
        NumDescs: 1,
        DescsLayout: d3d::D3D12_ELEMENTS_LAYOUT_ARRAY,
        Anonymous: d3d::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS_0 {
            pGeometryDescs: geometry,
        },
    }
}

impl EmptyTlas {
    pub(super) fn new(device: &d3d::ID3D12Device) -> Result<Self, crate::DeviceError> {
        let vertices = upload(
            device,
            bytemuck::cast_slice(&[[0_f32, 0., 0.], [1., 0., 0.], [0., 1., 0.]]),
        )?;
        let device5 = device
            .cast::<d3d::ID3D12Device5>()
            .into_device_result("Empty TLAS device")?;
        let geometry = geometry(unsafe { vertices.GetGPUVirtualAddress() });
        let inputs = inputs(&geometry);
        let mut sizes = d3d::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO::default();
        unsafe {
            device5.GetRaytracingAccelerationStructurePrebuildInfo(&inputs, &mut sizes);
        }
        let minimum = d3d::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BYTE_ALIGNMENT as u64;
        let blas = buffer(
            device,
            sizes.ResultDataMaxSizeInBytes.max(minimum),
            false,
            d3d::D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE,
        )?;
        let scratch = buffer(
            device,
            sizes.ScratchDataSizeInBytes.max(minimum),
            false,
            d3d::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )?;
        let instance = d3d::D3D12_RAYTRACING_INSTANCE_DESC {
            Transform: [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.],
            _bitfield1: 0,
            _bitfield2: 0,
            AccelerationStructure: unsafe { blas.GetGPUVirtualAddress() },
        };
        // This SDK struct has no padding; the zero instance mask keeps its valid BLAS invisible.
        let bytes = unsafe {
            core::slice::from_raw_parts(
                ptr::from_ref(&instance).cast::<u8>(),
                size_of::<d3d::D3D12_RAYTRACING_INSTANCE_DESC>(),
            )
        };
        let instances = upload(device, bytes)?;
        Ok(Self {
            vertices,
            blas,
            scratch,
            instances,
        })
    }

    /// Rebuild in each referencing command buffer, including buffers that are later discarded.
    pub(super) unsafe fn build(&self, list: &d3d::ID3D12GraphicsCommandList4) {
        let barrier = d3d::D3D12_RESOURCE_BARRIER {
            Type: d3d::D3D12_RESOURCE_BARRIER_TYPE_UAV,
            Anonymous: d3d::D3D12_RESOURCE_BARRIER_0 {
                UAV: core::mem::ManuallyDrop::new(d3d::D3D12_RESOURCE_UAV_BARRIER {
                    pResource: core::mem::ManuallyDrop::new(None),
                }),
            },
            ..Default::default()
        };
        unsafe {
            list.ResourceBarrier(core::slice::from_ref(&barrier));
            let geometry = geometry(self.vertices.GetGPUVirtualAddress());
            list.BuildRaytracingAccelerationStructure(
                &d3d::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
                    DestAccelerationStructureData: self.blas.GetGPUVirtualAddress(),
                    Inputs: inputs(&geometry),
                    ScratchAccelerationStructureData: self.scratch.GetGPUVirtualAddress(),
                    ..Default::default()
                },
                None,
            );
            list.ResourceBarrier(core::slice::from_ref(&barrier));
        }
    }
}

const _: () = assert!(size_of::<d3d::D3D12_RAYTRACING_INSTANCE_DESC>() == 64);
