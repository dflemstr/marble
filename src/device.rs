use std::sync;

use vulkano;
use vulkano_win;

use error;

const DEVICE_TYPE_PREF_ORDER: &'static [vulkano::instance::PhysicalDeviceType] =
    &[vulkano::instance::PhysicalDeviceType::DiscreteGpu,
      vulkano::instance::PhysicalDeviceType::IntegratedGpu,
      vulkano::instance::PhysicalDeviceType::VirtualGpu,
      vulkano::instance::PhysicalDeviceType::Cpu,
      vulkano::instance::PhysicalDeviceType::Other];

pub fn find_best<'a>(instance: &'a sync::Arc<vulkano::instance::Instance>,
                     window: &vulkano_win::Window)
                     -> error::Result<Option<vulkano::instance::PhysicalDevice<'a>>> {
    let mut devices = vulkano::instance::PhysicalDevice::enumerate(instance)
        .filter(|d| {
            d.queue_families()
            .any(|q| q.supports_graphics() && window.surface().is_supported(&q).unwrap_or(false))
        })
        .collect::<Vec<_>>();

    if devices.is_empty() {
        bail!(error::ErrorKind::NoDevicesAvailable)
    } else {
        devices.sort_by_key(|d| {
            DEVICE_TYPE_PREF_ORDER.iter()
                .position(|t| d.ty() == *t)
                .unwrap_or(DEVICE_TYPE_PREF_ORDER.len())
        });
        Ok(devices.into_iter().next())
    }
}
