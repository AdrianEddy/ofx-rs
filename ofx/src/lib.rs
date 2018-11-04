#![allow(unused)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![feature(concat_idents)]

extern crate ofx_sys;

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fmt;
use std::fmt::Display;
use std::marker::PhantomData;

mod types;
mod action;
mod prop;
mod result;
mod plugin;

pub use action::*;
pub use ofx_sys::*;
pub use result::*;
pub use types::*;
pub use plugin::*;


pub struct Registry {
	plugins: Vec<PluginDescriptor>,
	plugin_modules: HashMap<String, usize>,
}



impl Registry {
	pub fn new() -> Registry {
		Registry {
			plugin_modules: HashMap::new(),
			plugins: Vec::new(),
		}
	}

	pub fn add(
		&mut self,
		module_name: &'static str,
		name: &'static str,
		api_version: ApiVersion,
		plugin_version: PluginVersion,
		instance: Box<Execute>,
		set_host: SetHost,
		main_entry: MainEntry,
	) -> usize {
		let plugin_index = self.plugins.len();

		self.plugin_modules
			.insert(module_name.to_owned(), plugin_index as usize);

		let plugin = PluginDescriptor::new(
			plugin_index,
			module_name,
			name,
			api_version,
			plugin_version,
			instance,
			set_host,
			main_entry,
		);

		self.plugins.push(plugin);
		plugin_index
	}

	pub fn count(&self) -> Int {
		self.plugins.len() as Int
	}

	pub fn get_plugin_mut(&mut self, index: usize) -> &mut PluginDescriptor {
		&mut self.plugins[index as usize]
	}

	pub fn get_plugin(&self, index: usize) -> &PluginDescriptor {
		&self.plugins[index as usize]
	}

	pub fn ofx_plugin(&'static self, index: Int) -> &'static OfxPlugin {
		&self.plugins[index as usize].ofx_plugin()
	}

	pub fn dispatch(&mut self, plugin_module: &str, message: RawMessage) -> Result<Int> {
		println!("{}:{:?}", plugin_module, message);
		let found_plugin = self.plugin_modules.get(plugin_module).cloned();
		if let Some(plugin_index) = found_plugin {
			let plugin = self.get_plugin_mut(plugin_index);
			plugin.dispatch(message)
		} else {
			Err(Error::PluginNotFound)
		}
	}
}

pub fn set_host_for_plugin(plugin_module: &str, host: *mut OfxHost) {
	unsafe {
		get_registry_mut()
			.dispatch(plugin_module, RawMessage::SetHost { host: &*host })
			.ok();
	}
}

static mut global_registry: Option<Registry> = None;

pub fn main_entry_for_plugin(
	plugin_module: &str,
	action: CharPtr,
	handle: VoidPtr,
	in_args: OfxPropertySetHandle,
	out_args: OfxPropertySetHandle,
) -> Int {
	unsafe {
		get_registry_mut()
			.dispatch(
				plugin_module,
				RawMessage::MainEntry {
					action,
					handle,
					in_args,
					out_args,
				},
			)
			.ok()
			.unwrap_or(-1)
	}
}

pub fn init_registry<F>(init_function: F)
where
	F: Fn(&mut Registry),
{
	unsafe {
		if global_registry.is_none() {
			let mut registry = Registry::new();
			init_function(&mut registry);
			global_registry = Some(registry);
		}
	}
}

fn get_registry_mut() -> &'static mut Registry {
	unsafe { global_registry.as_mut().unwrap() }
}

pub fn get_registry() -> &'static Registry {
	unsafe { global_registry.as_ref().unwrap() }
}

#[macro_export]
macro_rules! plugin_module {
	($name:expr, $api_version:expr, $plugin_version:expr, $factory:expr) => {
		pub fn name() -> &'static str {
			$name
		}

		pub fn module_name() -> &'static str {
			module_path!().split("::").last().as_ref().unwrap()
		}

		pub fn new_instance() -> Box<Execute> {
			Box::new($factory())
		}

		pub fn api_version() -> ApiVersion {
			$api_version
		}

		pub fn plugin_version() -> PluginVersion {
			$plugin_version
		}

		pub extern "C" fn set_host(host: *mut ofx::OfxHost) {
			ofx::set_host_for_plugin(module_name(), host)
		}

		pub extern "C" fn main_entry(
			action: ofx::CharPtr,
			handle: ofx::VoidPtr,
			in_args: ofx::OfxPropertySetHandle,
			out_args: ofx::OfxPropertySetHandle,
		) -> super::Int {
			ofx::main_entry_for_plugin(module_name(), action, handle, in_args, out_args)
		}
	};
}

#[macro_export]
macro_rules! register_plugin {
	($registry:ident, $module:ident) => {
		$registry.add(
			$module::module_name(),
			$module::name(),
			$module::api_version(),
			$module::plugin_version(),
			$module::new_instance(),
			$module::set_host,
			$module::main_entry,
			);
	};
}

#[macro_export]
macro_rules! build_plugin_registry {
	($init_callback:ident) => {
		fn init() {
			init_registry($init_callback);
		}

		#[no_mangle]
		pub extern "C" fn OfxGetNumberOfPlugins() -> Int {
			init();
			get_registry().count()
		}

		#[no_mangle]
		pub extern "C" fn OfxGetPlugin(nth: Int) -> *const OfxPlugin {
			init();
			get_registry().ofx_plugin(nth) as *const OfxPlugin
		}

		pub fn describe_plugins() -> Vec<String> {
			unsafe {
				let n = OfxGetNumberOfPlugins();
				for i in 0..n {
					OfxGetPlugin(i);
				}
				(0..n)
					.map(|i| {
						let plugin = get_registry().get_plugin(i as usize);
						format!("{}", plugin)
					})
					.collect()
			}
		}
	};
}

#[macro_export]
macro_rules! register_modules {
	( $ ($module:ident), *) => {
		fn register_plugins(registry: &mut ofx::Registry) {
			$(register_plugin!(registry, $module);
			)*
		}

		build_plugin_registry!(register_plugins);
	};
}
