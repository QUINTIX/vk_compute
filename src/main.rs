#![allow(
    dead_code,
    unused_variables
)]

use std::collections::HashSet;
use std::fs;
use serde::Deserialize;
use anyhow::{anyhow, Result};
use owo_colors::{OwoColorize, AnsiColors};
use thiserror::Error;
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_1::*;

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

const VALIDATION_LAYER_RAW_STRING : &[u8] = b"VK_LAYER_KHRONOS_validation";
const VK_KHR_PORTABILITY_SUBSET_STR : &str = "VK_KHR_portability_subset";

const VALIDATION_LAYER: vk::ExtensionName =
	vk::ExtensionName::from_bytes(VALIDATION_LAYER_RAW_STRING);
const VK_KHR_PORTABILITY_SUBSET : vk::ExtensionName =
	vk::ExtensionName::from_bytes(VK_KHR_PORTABILITY_SUBSET_STR.as_bytes());

const HAS_COMPUTE : fn(&vk::QueueFamilyProperties) -> bool = 
	|p| p.queue_flags.contains(vk::QueueFlags::COMPUTE);

unsafe fn create_instance(entry: &Entry) -> Result<Instance>{
	let application_info = vk::ApplicationInfo::builder()
		.application_name(b"VKFromFileComputeSample\0")
		.application_version(vk::make_version(1, 0, 0))
		.engine_name(b"No Engine\0")
		.engine_version(vk::make_version(1, 0, 0))
		.api_version(vk::make_version(1, 1, 0));

	let available_layers = entry
		.enumerate_instance_layer_properties()?
		.iter()
		.map(|l| l.layer_name)
		.collect::<HashSet<_>>();

	if VALIDATION_ENABLED && !available_layers.contains(&VALIDATION_LAYER) {
		return Err(anyhow!("Validation layer requested but not supported."));
	}

	log_validation();

	let layers = configured_layers();
	
	let instance_create_info = vk::InstanceCreateInfo::builder()
		.application_info(&application_info)
		.enabled_layer_names(&layers);
	Ok(entry.create_instance(&instance_create_info, None)?)
}

unsafe fn get_first_compute_queue_family_index(
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
		Err(anyhow!(SuitabilityError("no suitable compute queue found")))
	}
}



#[derive(Clone, Debug)]
struct App {
	entry : Entry,
	instance: Instance,
	physical_device : vk::PhysicalDevice,
	logical_device : Device,
}

impl App {
	unsafe fn create(config : &DeviceConfig) -> Result<App> {
		let loader = LibloadingLoader::new(LIBRARY)?;
		let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
		let instance = create_instance(&entry)?;
		let physical_device = pick_physical_device(&instance, &config)?;
		
		let compute_queue_index = get_first_compute_queue_family_index(&instance, physical_device)?;
		let queue_priorities = &[1.0];
		let queue_infos = &[vk::DeviceQueueCreateInfo::builder()
			.queue_family_index(compute_queue_index)
			.queue_priorities(queue_priorities)];
		
		let layers = configured_layers();

		let does_have_portability_subset_extension =
			has_portability_subset_extension(&instance, physical_device)?;
		let extensions = if does_have_portability_subset_extension {
			 vec![VK_KHR_PORTABILITY_SUBSET.as_ptr()]
		} else {
			Vec::new()
		};

		let features = vk::PhysicalDeviceFeatures::builder();
		let mut more_features = vk::PhysicalDeviceFeatures2::builder().build();
		instance.get_physical_device_features2(physical_device, &mut more_features);
		
		let device_create_info_partial = vk::DeviceCreateInfo::builder()
			.queue_create_infos(queue_infos)
			.enabled_layer_names(&layers)
			.enabled_extension_names(&extensions);
		
		//required for shim'd Vulkan spec implementations, like MoltenVK
		let device_create_info = if does_have_portability_subset_extension {
			device_create_info_partial
			.push_next(&mut more_features)
			.build()
		} else {
			device_create_info_partial
			.enabled_features(&features)
			.build()
		};

		let logical_device = instance.create_device(physical_device, &device_create_info, None)?;
		
		Ok(Self { entry, instance, physical_device, logical_device })
	}

	unsafe fn destroy(&mut self) -> Result<()> {
		self.logical_device.destroy_device(None);
		self.instance.destroy_instance(None);
		Ok(())
	}
}

fn configured_layers() -> Vec<*const i8> {
	if VALIDATION_ENABLED {
		vec![VALIDATION_LAYER.as_ptr()]
	} else {
		Vec::new()
	}
}

fn log_validation() -> () {
	let validation_status = if VALIDATION_ENABLED {
		"ENABLED".color(AnsiColors::BrightWhite).on_color(AnsiColors::BrightBlue)
	} else {
		"DISABLED".color(AnsiColors::BrightWhite).on_color(AnsiColors::BrightGreen)
	};
	println!("debug extensions are {}", validation_status);
}

unsafe fn has_portability_subset_extension(
		instance: &Instance, physical_device : vk::PhysicalDevice) -> Result<bool> {
	let extension_properties = instance.enumerate_device_extension_properties(
		physical_device, None
	)?;
	let has_portability = extension_properties.iter()
		.map(|p| &p.extension_name)
		.map(|n| n.to_string_lossy())
		.any(|n| VK_KHR_PORTABILITY_SUBSET_STR == n);
	Ok(has_portability)
}

#[derive(Debug, Error)]
#[error("Missing {0}.")]
pub struct SuitabilityError(pub &'static str);

unsafe fn pick_physical_device(instance: &Instance, config: &DeviceConfig) -> Result<vk::PhysicalDevice> {
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
	Err(anyhow!(SuitabilityError("Failed to find suitable physical device.")))
}

unsafe fn has_compute_queue(instance: &Instance, physical_device : vk::PhysicalDevice) -> bool {
	let properties = instance.get_physical_device_queue_family_properties(physical_device);
	properties.iter().any(HAS_COMPUTE)
}

#[rustfmt::skip]
fn main() -> Result<()> {
	pretty_env_logger::init();
	
	let config = get_config()?;

	let mut app = unsafe { App::create(&config.device)? };
	
	// stuff happens here

	unsafe { app.destroy() }
}

#[derive(Deserialize)]
struct Config {
	device : DeviceConfig,
}

#[derive(Deserialize)]
struct DeviceConfig {
	first_device : bool,
	device_id : Option<u32>,
}

fn get_config() ->  Result<Config, toml::de::Error> {
	let contents = fs::read_to_string("config.toml")
		.expect("couldn't load config.toml");
	toml::from_str(&contents)
}
