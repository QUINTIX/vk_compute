use vulkanalia::prelude::v1_1::*;
use anyhow::{anyhow, Result};
use thiserror::Error;
use owo_colors::{OwoColorize};
use std::fs;
use serde::Deserialize;

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

const HAS_COMPUTE : fn(&vk::QueueFamilyProperties) -> bool = 
	|p| p.queue_flags.contains(vk::QueueFlags::COMPUTE);

pub unsafe fn pick_physical_device(instance: &Instance, config: &DeviceConfig) -> Result<vk::PhysicalDevice> {
	for physical_device in instance.enumerate_physical_devices()? {
		let props = instance.get_physical_device_properties(physical_device);
		println!("found device with vendor_id {:x} and device_id {:x} that is named {}", 
			(props.vendor_id).green(),
			(props.device_id).green(), 
			(props.device_name).bright_blue()
		);
		
		if !has_compute_queue(&instance, physical_device){
			continue;
		}

		if config.first_device {
			println!("using first available device {}", (props.device_name).bright_blue());
			return Ok(physical_device)
		} else if None == config.device_id {
			return Err(anyhow!("must specify either a device_id or first_device"))
		} else if props.device_id == config.device_id.unwrap() {
			println!("using selected device {}", (props.device_name).bright_blue());
			return Ok(physical_device)
		}
	}
	Err(anyhow!(SuitabilityError("suitable physical device")))
}

pub unsafe fn has_compute_queue(instance: &Instance, physical_device : vk::PhysicalDevice) -> bool {
	let properties = instance.get_physical_device_queue_family_properties(physical_device);
	properties.iter().any(HAS_COMPUTE)
}

#[derive(Deserialize)]
pub struct Config {
	pub device : DeviceConfig,
}

#[derive(Deserialize)]
pub struct DeviceConfig {
	first_device : bool,
	device_id : Option<u32>,
}

pub fn get_config() ->  Result<Config, toml::de::Error> {
	let contents = fs::read_to_string("config.toml")
		.expect("couldn't load config.toml");
	toml::from_str(&contents)
}

pub unsafe fn get_first_compute_queue_family_index(
	instance: &Instance,
	physical_device: vk::PhysicalDevice
) -> Result<u32> {
	let properties = instance
		.get_physical_device_queue_family_properties(physical_device);
	
	let maybe_index = properties.iter()
		.position(HAS_COMPUTE)
		.map(|i| i as u32);

	if let Some(maybe_index) = maybe_index {
		Ok(maybe_index)
	} else {
		Err(anyhow!(SuitabilityError("suitable compute queue")))
	}
}

pub unsafe fn create_shader_module(
    device: &Device,
    bytecode: &[u8],
) -> Result<vk::ShaderModule> {
	let bytecode = Vec::<u8>::from(bytecode);
	let (prefix, code, suffix) = bytecode.align_to::<u32>();
	if !prefix.is_empty() || !suffix.is_empty() {
		return Err(anyhow!("Shader bytecode is not properly aligned."));
	}

	let info = vk::ShaderModuleCreateInfo::builder()
    .code_size(bytecode.len())
    .code(code);

	Ok(device.create_shader_module(&info, None)?)
}
