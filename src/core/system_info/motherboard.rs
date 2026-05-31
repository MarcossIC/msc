use crate::core::system_info::types::MotherboardInfo;
use crate::error::Result;
use std::time::{Duration, Instant};

#[cfg(windows)]
use crate::platform::system::windows::mbo::{
    detect_chipset, detect_secure_boot, detect_tpm_version, get_baseboard_bios,
    get_dimm_slot_count, get_motherboard_info,
};

pub fn collect() -> Result<MotherboardInfo> {
    #[cfg(windows)]
    {
        get_motherboard_info()
    }

    #[cfg(not(windows))]
    {
        // On Linux, could read from /sys/class/dmi/id/
        // On macOS, could use system_profiler
        Err(crate::error::MscError::other(
            "Not implemented on this platform",
        ))
    }
}

/// Collect motherboard info AND per-sub-query timings.
/// On non-Windows targets, returns empty subs.
pub fn collect_with_subs() -> Result<(MotherboardInfo, Vec<(String, Duration)>)> {
    #[cfg(windows)]
    {
        let mut subs = Vec::with_capacity(4);

        let t = Instant::now();
        let bb = get_baseboard_bios();
        subs.push(("motherboard.baseboard+bios".to_string(), t.elapsed()));

        let t = Instant::now();
        let chipset = detect_chipset();
        subs.push(("motherboard.chipset".to_string(), t.elapsed()));

        let t = Instant::now();
        let tpm_version = detect_tpm_version();
        subs.push(("motherboard.tpm".to_string(), t.elapsed()));

        let t = Instant::now();
        let secure_boot = detect_secure_boot();
        subs.push(("motherboard.secure_boot".to_string(), t.elapsed()));

        let t = Instant::now();
        let dimm_slots = get_dimm_slot_count();
        subs.push(("motherboard.dimm_slots".to_string(), t.elapsed()));

        let info = MotherboardInfo {
            manufacturer: bb.manufacturer,
            product: bb.product,
            version: bb.version,
            bios_vendor: bb.bios_vendor,
            bios_version: bb.bios_version,
            chipset,
            tpm_version,
            secure_boot,
            dimm_slots,
            pcie_slots: None,
            m2_slots_total: None,
            m2_slots_used: None,
        };

        Ok((info, subs))
    }

    #[cfg(not(windows))]
    {
        collect().map(|info| (info, Vec::new()))
    }
}
