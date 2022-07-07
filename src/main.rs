#![allow(
    dead_code,
    unused_variables
)]

mod lib;

use std::collections::HashSet;
use std::ptr::copy_nonoverlapping as memcpy;
use std::mem::size_of;

use anyhow::{anyhow, Result};
use owo_colors::{OwoColorize, AnsiColors};
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_1::*;
use lib::{
	get_config,
	Config,
	DeviceConfig,
	pick_physical_device,
	get_first_compute_queue_family_index, 
	get_best_memory_type_index,
	create_shader_module
};

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

const VK_KHR_PORTABILITY_SUBSET_STR : &str = "VK_KHR_portability_subset";

const VALIDATION_LAYER: vk::ExtensionName =
	vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");
const VK_KHR_PORTABILITY_SUBSET : vk::ExtensionName =
	vk::ExtensionName::from_bytes(VK_KHR_PORTABILITY_SUBSET_STR.as_bytes());

const NUM_FLOATS : usize = 16384;
const NUM_BUFFERS : usize = 2;

unsafe fn create_instance(entry: &Entry) -> Result<Instance>{
	let application_info = vk::ApplicationInfo::builder()
		.application_name(b"VKFromFileComputeSample\0")
		.application_version(vk::make_version(1, 0, 0))
		.engine_name(b"No Engine\0")
		.engine_version(vk::make_version(1, 0, 0))
		.api_version(vk::make_version(1, 1, 0))
	.build();

	let available_layers = entry
		.enumerate_instance_layer_properties()?
		.iter()
		.map(|l| l.layer_name)
		.collect::<HashSet<_>>();

	if VALIDATION_ENABLED && !available_layers.contains(&VALIDATION_LAYER) {
		return Err(anyhow!("Validation layer requested but not supported."));
	}

	log_validation();

	let layers = if VALIDATION_ENABLED {
		vec![VALIDATION_LAYER.as_ptr()]
	} else {
		Vec::new()
	};
	
	let instance_create_info = vk::InstanceCreateInfo::builder()
		.application_info(&application_info)
		.enabled_layer_names(&layers)
	.build();
	Ok(entry.create_instance(&instance_create_info, None)?)
}

#[derive(Clone, Debug)]
struct App {
	entry : Entry,
	instance: Instance,
	physical_device : vk::PhysicalDevice,
	logical_device : Device,
	queue_index : u32,
	memory_index : u32,
	memory: vk::DeviceMemory,
	compute_shader: vk::ShaderModule,
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
			.queue_priorities(queue_priorities).build()];
		
		let layers = if VALIDATION_ENABLED {
			vec![VALIDATION_LAYER.as_ptr()]
		} else {
			Vec::new()
		};

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
		
		let shader_binary = std::include_bytes!("../compute.spv");
		let compute_shader = create_shader_module(&logical_device, shader_binary)?;

		let memory_propertes = instance.get_physical_device_memory_properties(physical_device);
		let desired_size = (NUM_BUFFERS * NUM_FLOATS * size_of::<f32>()) as vk::DeviceSize;

		let memory_index : u32 = get_best_memory_type_index(
			&memory_propertes, 
			vk::MemoryPropertyFlags::HOST_COHERENT |
			vk::MemoryPropertyFlags::HOST_VISIBLE,
			desired_size as usize
		)?;

		let memory_allocate_info = vk::MemoryAllocateInfo::builder()
			.allocation_size(desired_size)
			.memory_type_index(memory_index)
		.build();

		let memory = logical_device.allocate_memory(
			&memory_allocate_info, None)?;

		let queue_index : u32 = compute_queue_index;
		
		Ok(Self { 
			entry, instance, 
			physical_device, logical_device, 
			queue_index, memory_index,
			memory,
			compute_shader
		})
	}

	pub unsafe fn populate_buffer(&mut self) -> Result<()> {
		let mut floats : Vec<f32 >= Vec::with_capacity(NUM_FLOATS);

		for item in 0..NUM_FLOATS {
			floats.push((item as f32) * 0.5);
		}

		let shader_read_buffer_size = (NUM_FLOATS * size_of::<f32>()) as vk::DeviceSize;
		let mapped = self.logical_device.map_memory(
			self.memory, 0, shader_read_buffer_size, vk::MemoryMapFlags::empty()
		)?;

		memcpy(floats.as_ptr(), mapped.cast(), floats.len());

		self.logical_device.unmap_memory(self.memory);

		Ok(())
	}

	pub unsafe fn bind_buffer_layout(&mut self) -> Result<(
		vk::Buffer, vk::Buffer, vk::DescriptorSetLayout
	)> {
		let size_and_offset = (NUM_FLOATS * size_of::<f32>()) as vk::DeviceSize;

		let buffer_info = vk::BufferCreateInfo::builder()
			.size(size_and_offset)
			.usage(vk::BufferUsageFlags::STORAGE_BUFFER)
			.sharing_mode(vk::SharingMode::EXCLUSIVE)
		.build();

		let in_buffer = self.logical_device.create_buffer(&buffer_info, None)?;
		self.logical_device.bind_buffer_memory(
			in_buffer, self.memory, 0
		)?;

		let out_buffer = self.logical_device.create_buffer(&buffer_info, None)?;
		self.logical_device.bind_buffer_memory(
			out_buffer, self.memory, size_and_offset
		)?;

		let bindings : Vec<vk::DescriptorSetLayoutBinding> = vec![
			vk::DescriptorSetLayoutBinding::builder()
				.binding(0)
				.descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
				.descriptor_count(1)
				.stage_flags(vk::ShaderStageFlags::COMPUTE)
			.build(),
			vk::DescriptorSetLayoutBinding::builder()
				.binding(1)
				.descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
				.descriptor_count(1)
				.stage_flags(vk::ShaderStageFlags::COMPUTE)
			.build(),
		];

		let info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);
		let layout = self.logical_device.create_descriptor_set_layout(&info, None)?;

		Ok((in_buffer, out_buffer, layout))
	}

	pub unsafe fn create_pipeine_with_layout(&mut self, 
			descriptor_layout : &vk::DescriptorSetLayout
	) -> Result<(vk::Pipeline, vk::PipelineLayout)> {
		let descriptor_layout_wrapped = &[*descriptor_layout];

		let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::builder()
			.set_layouts(descriptor_layout_wrapped)
		.build();

		let pipeline_layout = self.logical_device.create_pipeline_layout(
			&pipeline_layout_create_info, None)?;

		let compute_pipeline_create_info = vk::ComputePipelineCreateInfo::builder()
			.stage(vk::PipelineShaderStageCreateInfo::builder()
				.stage(vk::ShaderStageFlags::COMPUTE)
				.module(self.compute_shader)
				.name(b"main\0")
			.build())
			.layout(pipeline_layout)
		.build();

		let (pipeline, _) = self.logical_device.create_compute_pipelines(
			vk::PipelineCache::default(), &[compute_pipeline_create_info],
			None)?;

		Ok((pipeline, pipeline_layout))
	}

	pub unsafe fn create_command_pool_and_buffer(&mut self) -> Result<(
			vk::CommandPool, vk::CommandBuffer
	)> {
		let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
			.queue_family_index(self.queue_index).build();
		let command_pool = self.logical_device.create_command_pool(
			&command_pool_create_info, None)?;

		let command_buffer_alloc_info = vk::CommandBufferAllocateInfo::builder()
			.command_pool(command_pool)
			.level(vk::CommandBufferLevel::PRIMARY)
			.command_buffer_count(1)
		.build();

		let mut command_buffers = self.logical_device.allocate_command_buffers(
			&command_buffer_alloc_info)?;

		Ok((command_pool, command_buffers.remove(0)))
	}

	pub unsafe fn record_commands_to_buffer(&mut self, 
			command_buffer : &vk::CommandBuffer,
			pipeline : &vk::Pipeline,
			pipeline_layout : &vk::PipelineLayout,
			descriptor_set : &vk::DescriptorSet
	) -> Result<(), vk::ErrorCode> {
		let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
				.flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT).build();

		self.logical_device.begin_command_buffer(*command_buffer,
			&command_buffer_begin_info)?;

		self.logical_device.cmd_bind_pipeline(
			*command_buffer, vk::PipelineBindPoint::COMPUTE, *pipeline);

		self.logical_device.cmd_bind_descriptor_sets(
			*command_buffer, vk::PipelineBindPoint::COMPUTE,
			*pipeline_layout, 0, &[*descriptor_set], &[]
		);

		self.logical_device.cmd_dispatch(*command_buffer, NUM_FLOATS as u32, 1, 1);

		self.logical_device.end_command_buffer(*command_buffer)
	}

	unsafe fn destroy(&mut self,
			command_pool : vk::CommandPool,
			in_buffer : vk::Buffer,
			out_buffer : vk::Buffer,
			descriptor_layout : vk::DescriptorSetLayout,
			pipeline : vk::Pipeline,
			pipeline_layout : vk::PipelineLayout
	) -> Result<()> {
		self.logical_device.destroy_command_pool(command_pool, None);
		self.logical_device.destroy_pipeline(pipeline, None);
		self.logical_device.destroy_pipeline_layout(pipeline_layout, None);
		self.logical_device.destroy_shader_module(self.compute_shader, None);
		self.logical_device.destroy_buffer(in_buffer, None);
		self.logical_device.destroy_buffer(out_buffer, None);
		self.logical_device.destroy_descriptor_set_layout(descriptor_layout, None);
		self.logical_device.free_memory(self.memory, None);
		self.logical_device.destroy_device(None);
		self.instance.destroy_instance(None);
		Ok(())
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
		physical_device, None)?;

	let has_portability = extension_properties.iter()
		.map(|p| &p.extension_name)
		.map(|n| n.to_string_lossy())
		.any(|n| VK_KHR_PORTABILITY_SUBSET_STR == n);
	Ok(has_portability)
}

#[rustfmt::skip]
fn main() -> Result<()> {
	pretty_env_logger::init();
	
	let Config {device : device_config} = get_config()?;

	let mut app = unsafe { App::create(&device_config)? };
	println!("found compute index {} and memory index {}", 
		(app.queue_index).green(), (app.memory_index).green());

	unsafe { app.populate_buffer()? };
	let (in_buffer, out_buffer, descriptor_layout) = unsafe {
		app.bind_buffer_layout()? };

	let (pipeline, pipeline_layout) = unsafe {
		app.create_pipeine_with_layout(&descriptor_layout)? };

	let (command_pool, command_buffer) = unsafe {
		app.create_command_pool_and_buffer()? };
	
	// unsafe { app.record_commands_to_buffer(
	// 	&command_buffer,
	// 	&pipeline,
	// 	&pipeline_layout,
	// 	&descriptor_layout
	// )};

	// stuff happens here

	unsafe { 
		app.destroy(
			command_pool,
			in_buffer, out_buffer,
			descriptor_layout,
			pipeline, pipeline_layout
		)
	}
}
