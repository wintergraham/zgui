//! What a bug report says of the machine's graphics: every GPU adapter the
//! system has, and the one zgx draws with and how.

/// A GPU adapter of the system.
#[derive(Clone, Debug)]
pub struct Adapter {
    /// The adapter's name, as the system gives it.
    pub name: String,
    /// PCI vendor id (0 where the system gives none).
    pub vendor: u32,
    /// PCI device id (0 where the system gives none).
    pub device: u32,
    /// The driver's version, as the system reports it (empty if unknown).
    pub driver: String,
    /// "integrated", "discrete", "software", "virtual" or "unknown".
    pub kind: &'static str,
}

/// The adapter zgx draws with, and the paths it takes on it.
#[derive(Clone, Debug)]
pub struct InUse {
    /// The adapter, as its device describes it.
    pub adapter: Adapter,
    /// The graphics API: "Dx12", "Metal", "Vulkan" or "Gl".
    pub backend: String,
    /// Uploads written in place in memory the CPU and GPU share.
    pub mapped_uploads: bool,
    /// Per-frame vertices written in place in a ring of buffers the GPU reads.
    pub vertex_ring: bool,
    /// Windows: whether the last window made presents through a swap chain of
    /// zgx's own (dxgi_present.rs) rather than wgpu's. None elsewhere.
    pub own_swap_chain: Option<bool>,
}

static IN_USE: std::sync::Mutex<Option<InUse>> = std::sync::Mutex::new(None);

fn kind(device_type: wgpu::DeviceType) -> &'static str {
    match device_type {
        wgpu::DeviceType::IntegratedGpu => "integrated",
        wgpu::DeviceType::DiscreteGpu => "discrete",
        wgpu::DeviceType::Cpu => "software",
        wgpu::DeviceType::VirtualGpu => "virtual",
        wgpu::DeviceType::Other => "unknown",
    }
}

/// zgx made its device on `info`'s adapter.
pub(crate) fn chose(info: &wgpu::AdapterInfo, mapped_uploads: bool, vertex_ring: bool) {
    let adapter = Adapter {
        name: info.name.clone(),
        vendor: info.vendor,
        device: info.device,
        driver: [info.driver.as_str(), info.driver_info.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        kind: kind(info.device_type),
    };
    let mut slot = IN_USE.lock().unwrap_or_else(|e| e.into_inner());
    let own_swap_chain = slot.as_ref().and_then(|used| used.own_swap_chain);
    *slot = Some(InUse {
        adapter,
        backend: format!("{:?}", info.backend),
        mapped_uploads,
        vertex_ring,
        own_swap_chain,
    });
}

/// A window's renderer presents through a swap chain of its own, or not.
#[cfg(windows)]
pub(crate) fn presenting(own_swap_chain: bool) {
    if let Some(used) = IN_USE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        used.own_swap_chain = Some(own_swap_chain);
    }
}

/// The adapter zgx draws with, once it made a device.
pub fn in_use() -> Option<InUse> {
    IN_USE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Every GPU adapter of the system, without opening a device on any (a
/// driver keeps memory for each one opened). Windows: DXGI's list, whose
/// kind is a guess from the adapter's own memory, since only a device says
/// for certain (the one zgx draws with: `in_use`).
#[cfg(windows)]
pub fn adapters() -> Vec<Adapter> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIDevice, IDXGIFactory1,
    };
    use windows::core::Interface;
    let mut adapters = Vec::new();
    // SAFETY: Plain DXGI calls on objects made here; only read.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return adapters;
        };
        let mut index = 0;
        while let Ok(adapter) = factory.EnumAdapters1(index) {
            index += 1;
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
            let name = String::from_utf16_lossy(&desc.Description[..len]);
            // The user-mode driver's version, in four 16-bit parts.
            let driver = adapter
                .CheckInterfaceSupport(&IDXGIDevice::IID)
                .map(|v| {
                    let v = v as u64;
                    format!("{}.{}.{}.{}", v >> 48, (v >> 32) & 0xFFFF, (v >> 16) & 0xFFFF, v & 0xFFFF)
                })
                .unwrap_or_default();
            const MIB: usize = 1024 * 1024;
            let kind = if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                "software"
            } else if desc.DedicatedVideoMemory <= 512 * MIB {
                "integrated"
            } else {
                "discrete"
            };
            adapters.push(Adapter {
                name: format!("{name} ({} MiB own memory)", desc.DedicatedVideoMemory / MIB),
                vendor: desc.VendorId,
                device: desc.DeviceId,
                driver,
                kind,
            });
        }
    }
    adapters
}

/// Every GPU adapter of the system (Metal's devices; elsewhere wgpu's list).
#[cfg(not(windows))]
pub fn adapters() -> Vec<Adapter> {
    let backends = if cfg!(target_vendor = "apple") {
        wgpu::Backends::METAL
    } else {
        wgpu::Backends::VULKAN
    };
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = backends;
    let instance = wgpu::Instance::new(descriptor);
    pollster::block_on(instance.enumerate_adapters(backends))
        .into_iter()
        .map(|adapter| {
            let info = adapter.get_info();
            Adapter {
                name: info.name,
                vendor: info.vendor,
                device: info.device,
                driver: [info.driver, info.driver_info]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
                kind: kind(info.device_type),
            }
        })
        .collect()
}
