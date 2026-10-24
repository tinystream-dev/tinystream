// SPDX-License-Identifier: AGPL-3.0-or-later

//! The wgpu device, opened with what mpv's frames need to reach wgpu without a copy (#86): Vulkan
//! memory and semaphores that GL can import.

use std::ffi::CStr;

use ash::khr;
use wgpu::hal::api::Vulkan;

const EXTENSIONS: [&CStr; 2] = [khr::external_memory_fd::NAME, khr::external_semaphore_fd::NAME];

pub fn request_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<(wgpu::Device, wgpu::Queue)> {
    // SAFETY: the hal adapter is only used to open the device below, while `adapter` is alive.
    let hal = unsafe { adapter.as_hal::<Vulkan>() }?;

    let capabilities = hal.physical_device_capabilities();
    let missing: Vec<_> = EXTENSIONS.into_iter().filter(|name| !capabilities.supports_extension(name)).collect();
    if !missing.is_empty() {
        tracing::warn!("the GPU driver lacks {missing:?}; video will need a slower path");
        return None;
    }

    // SAFETY: the callback only adds extensions the device supports, and removes nothing.
    let device = unsafe {
        hal.open_with_callback(
            descriptor.required_features,
            &descriptor.required_limits,
            &descriptor.memory_hints,
            Some(Box::new(|args| {
                for name in EXTENSIONS {
                    if !args.extensions.contains(&name) {
                        args.extensions.push(name);
                    }
                }
            })),
        )
    };
    let device = match device {
        Ok(device) => device,
        Err(error) => {
            tracing::warn!("can't open the Vulkan device with external memory: {error}");
            return None;
        },
    };
    drop(hal);

    // SAFETY: `device` was opened from this adapter with the descriptor's features.
    match unsafe { adapter.create_device_from_hal(device, descriptor) } {
        Ok(device) => {
            tracing::info!("Vulkan device opened with external memory and semaphores");
            Some(device)
        },
        Err(error) => {
            tracing::warn!("can't use the Vulkan device with external memory: {error}");
            None
        },
    }
}
