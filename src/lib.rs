use vulkanalia::prelude::v1_1::*;
use anyhow::{anyhow, Result};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Missing {0}.")]
pub struct SuitabilityError(pub &'static str);


pub fn get_best_memory_type_index(
		properties: &vk::PhysicalDeviceMemoryProperties,
		desired_flags: vk::MemoryPropertyFlags,
		desired_size: usize
) -> Result<u32> {
	(0..properties.memory_type_count)
		.find(|i| {
			let memory_type = properties.memory_types[*i as usize];
			let memory_heap = properties.memory_heaps[memory_type.heap_index as usize];
			let right_properties = memory_type.property_flags.contains(desired_flags);
			let right_size = desired_size as u64 <= memory_heap.size;
			right_properties && right_size
	}).ok_or_else(|| anyhow!(SuitabilityError("memory type")))
}
