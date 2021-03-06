use crate::error::Error;
use bevy_openxr_core::{set_xr_instance, XrInstance};
use openxr::{ExtensionSet, Instance};

// Platform-specific loaders
#[cfg(target_os = "android")] // FIXME change only for oculus instead of android
pub mod oculus_android;

// Loader trait, can be overridden
pub(crate) trait OpenXRInstance {
    fn load_bevy_openxr() -> Result<openxr::Entry, Error> {
        panic!("OpenXRInstance::load_bevy_openxr unimplemented for this platform");
    }

    fn instantiate(&mut self, _extensions: &mut ExtensionSet) -> Result<Instance, Error> {
        panic!("OpenXRInstance::instantiate unimplemented for this platform");
    }
}

// Default
#[cfg(not(target_os = "android"))] // FIXME use platform_oculus_android?
impl OpenXRInstance for openxr::Entry {
    fn load_bevy_openxr() -> Result<openxr::Entry, Error> {
        // FIXME: use ::load by default, path from config?
        Ok(openxr::Entry::load()?)
    }

    fn instantiate(&mut self, extensions: &mut ExtensionSet) -> Result<Instance, Error> {
        let app_info = &openxr::ApplicationInfo {
            application_name: "hello openxr",
            engine_name: "bevy",
            application_version: 1, // FIXME allow user to submit application version?
            engine_version: 1,      // FIXME pull bevy version from somewhere?
        };

        let xr_instance = self
            .create_instance(app_info, &extensions, None, &[])
            .unwrap();

        Ok(xr_instance)
    }
}

pub(crate) fn initialize_openxr() {
    let mut entry = match openxr::Entry::load_bevy_openxr() {
        Ok(entry) => entry,
        Err(_) => {
            println!("Could not load openxr loader. Make sure that you have openxr_loader.dll (Windows), libopenxr_loader.dylib (MacOS) or libopenxr_loader.so (Linux) in the library load path");
            std::process::exit(255);
        }
    };
    let mut extensions = entry.enumerate_extensions().unwrap();

    // because of https://gitlab.freedesktop.org/monado/monado/-/issues/98
    extensions.mnd_headless = false;

    let instance = entry.instantiate(&mut extensions).unwrap();
    let wgpu_openxr = wgpu::wgpu_openxr::new(
        wgpu::BackendBit::VULKAN,
        &instance,
        wgpu::wgpu_openxr::OpenXROptions::default(),
    )
    .unwrap();

    set_xr_instance(XrInstance::new(wgpu_openxr, instance));
}
