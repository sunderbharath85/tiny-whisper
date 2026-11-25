//! Which GPU, if any, transcription can use.
//!
//! GPU builds (`--features metal` / `vulkan`) also run on machines without a
//! usable GPU: the Windows installer ships the Vulkan loader, so the app
//! starts without a GPU driver, and whisper.cpp then finds no GPU device and
//! runs on the CPU. The GPU setting is treated as CPU for every engine in that
//! case, so ONNX Runtime doesn't fall back to DirectML on Windows' software
//! renderer, which is far slower than its CPU path.

use crate::config::Device;
use std::sync::OnceLock;

/// The first GPU whisper.cpp would use, as "name (description)", or `None`
/// when this build has no GPU backend or the machine has no usable GPU.
/// Detected once, on first use.
pub fn detected() -> Option<&'static str> {
    static GPU: OnceLock<Option<String>> = OnceLock::new();
    GPU.get_or_init(detect).as_deref()
}

/// The device to run on: the GPU setting falls back to the CPU when no GPU
/// was found.
pub fn effective(requested: Device) -> Device {
    match requested {
        Device::Gpu if detected().is_some() => Device::Gpu,
        _ => Device::Cpu,
    }
}

/// Mirrors `whisper_backend_init_gpu` in whisper.cpp: the first device of
/// type GPU or IGPU among ggml's registered backends.
#[cfg(feature = "gpu")]
fn detect() -> Option<String> {
    use std::ffi::CStr;
    use whisper_rs::whisper_rs_sys as sys;

    let text = |p: *const std::os::raw::c_char| {
        if p.is_null() {
            String::new()
        } else {
            // SAFETY: ggml returns NUL-terminated strings owned by the device.
            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
        }
    };
    // SAFETY: plain queries of ggml's backend registry, which initializes
    // itself on first use. A backend that fails to start (e.g. Vulkan with no
    // driver) registers no devices instead of failing.
    unsafe {
        for i in 0..sys::ggml_backend_dev_count() {
            let dev = sys::ggml_backend_dev_get(i);
            let ty = sys::ggml_backend_dev_type(dev);
            if ty == sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU
                || ty == sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_IGPU
            {
                let name = text(sys::ggml_backend_dev_name(dev));
                let desc = text(sys::ggml_backend_dev_description(dev));
                return Some(if desc.is_empty() || desc == name {
                    name
                } else {
                    format!("{name} ({desc})")
                });
            }
        }
    }
    None
}

#[cfg(not(feature = "gpu"))]
fn detect() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_setting_stays_cpu() {
        assert_eq!(effective(Device::Cpu), Device::Cpu);
    }

    #[test]
    fn gpu_setting_follows_detection() {
        eprintln!("detected GPU: {:?}", detected());
        let want = if detected().is_some() { Device::Gpu } else { Device::Cpu };
        assert_eq!(effective(Device::Gpu), want);
    }
}
