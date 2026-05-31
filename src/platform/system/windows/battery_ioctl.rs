//! Native battery cycle-count reader (`IOCTL_BATTERY_QUERY_INFORMATION`).
//!
//! Reads `BATTERY_INFORMATION.CycleCount` straight from the battery device via
//! SetupAPI enumeration + two IOCTLs against the in-box battery class driver —
//! no WMI, no external process. This is the source IMPROVEMENTS-V2 item 7 asks
//! for: `Win32_Battery` exposes no cycle count at all, and while the `root\wmi`
//! `BatteryCycleCount` class does, this native path is the documented primary.
//!
//! Honesty rule (the project's non-negotiable):
//!   - `CycleCount == 0` means the firmware does NOT report it. We return `None`,
//!     never a misleading "0 cycles" (which reads as "brand new").
//!   - ANY failure (no battery, desktop, IOCTL rejected, access denied) → `None`.
//!
//! Design (mirrors `nvme_smart.rs` / `pcie_link.rs`):
//!   - [`parse_battery_information`] is PURE — reads a `BATTERY_INFORMATION`
//!     buffer by offset, golden-tested, no hardware. That's where an offset or
//!     endianness bug would fabricate data, so it's isolated and covered.
//!   - [`read_battery_cycle_count`] is the isolated `unsafe` fetch.

use std::ffi::c_void;
use std::ptr;

use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

// --- Constants defined locally (the SetupAPI / DeviceIoControl features are
// already enabled for pcie_link.rs / nvme_smart.rs, so no Cargo.toml change).
// Verified against windows-sys 0.61.2 and the Win32 battery headers. ---

/// {72631E54-78A4-11D0-BCF7-00AA00B7B32A} — `GUID_DEVICE_BATTERY` (battery
/// device interface class). Local to avoid pulling another feature.
const GUID_DEVICE_BATTERY: GUID = GUID::from_u128(0x72631e54_78a4_11d0_bcf7_00aa00b7b32a);

/// `GENERIC_READ`/`GENERIC_WRITE` — the battery IOCTLs carry `FILE_READ_ACCESS`,
/// so (unlike the storage attribute queries) the handle needs real access. This
/// still does NOT require admin for the battery class.
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;

/// `IOCTL_BATTERY_QUERY_TAG` = CTL_CODE(FILE_DEVICE_BATTERY=0x29, 0x10,
/// METHOD_BUFFERED, FILE_READ_ACCESS) = 0x00294040.
const IOCTL_BATTERY_QUERY_TAG: u32 = 0x0029_4040;
/// `IOCTL_BATTERY_QUERY_INFORMATION` = CTL_CODE(0x29, 0x11, METHOD_BUFFERED,
/// FILE_READ_ACCESS) = 0x00294044.
const IOCTL_BATTERY_QUERY_INFORMATION: u32 = 0x0029_4044;

/// `BatteryInformation` level of the `BATTERY_QUERY_INFORMATION_LEVEL` enum.
const BATTERY_INFORMATION_LEVEL: i32 = 0;

/// `sizeof(BATTERY_INFORMATION)` — see field offsets in [`parse_battery_information`].
const BATTERY_INFORMATION_SIZE: usize = 36;

/// Input struct for `IOCTL_BATTERY_QUERY_INFORMATION` (`BATTERY_QUERY_INFORMATION`).
#[repr(C)]
struct BatteryQueryInformation {
    battery_tag: u32,
    information_level: i32,
    at_rate: i32,
}

/// Parsed subset of `BATTERY_INFORMATION`.
#[derive(Debug, Clone, PartialEq)]
// designed/full capacity are parsed for the diagnostic dump and future wear math
// off the IOCTL; the live path keeps using WMI capacities (reliably in mWh).
#[allow(dead_code)]
pub struct BatteryInformation {
    pub capabilities: u32,
    pub designed_capacity: u32,
    pub full_charged_capacity: u32,
    pub cycle_count: u32,
}

impl BatteryInformation {
    /// Honest cycle count: `None` when the firmware reports 0 (= not supported),
    /// so we never render a misleading "0 cycles".
    pub fn cycle_count_opt(&self) -> Option<u32> {
        if self.cycle_count == 0 {
            None
        } else {
            Some(self.cycle_count)
        }
    }
}

/// Parse a `BATTERY_INFORMATION` buffer (36 bytes, all little-endian).
///
/// Pure: no I/O, no `unsafe`. Returns `None` if the buffer is too short. Field
/// offsets (`#[repr(C)]`, ULONG=4 / UCHAR=1):
///   0   Capabilities (u32)        4   Technology (u8)   5   Reserved[3]
///   8   Chemistry[4]              12  DesignedCapacity (u32)
///   16  FullChargedCapacity (u32) 20  DefaultAlert1     24  DefaultAlert2
///   28  CriticalBias              32  CycleCount (u32)
pub fn parse_battery_information(buf: &[u8]) -> Option<BatteryInformation> {
    if buf.len() < BATTERY_INFORMATION_SIZE {
        return None;
    }
    let u32_at = |off: usize| u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
    Some(BatteryInformation {
        capabilities: u32_at(0),
        designed_capacity: u32_at(12),
        full_charged_capacity: u32_at(16),
        cycle_count: u32_at(32),
    })
}

/// Read the battery cycle count via the native IOCTL path.
///
/// Returns the first present battery's cycle count, or `None` if there is no
/// battery, the driver rejects the IOCTL, or the firmware reports 0. Never
/// fabricates a value. Safe wrapper around the isolated `unsafe` block.
pub fn read_battery_cycle_count() -> Option<u32> {
    // SAFETY: SetupAPI interface enumeration. The device-info set and every
    // opened handle are released on every return path.
    unsafe {
        let set = SetupDiGetClassDevsW(
            &GUID_DEVICE_BATTERY,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_DEVICEINTERFACE | DIGCF_PRESENT,
        );
        if set == INVALID_HANDLE_VALUE as isize {
            return None;
        }

        let mut result = None;
        let mut index: u32 = 0;
        loop {
            let mut iface: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
            iface.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            // null device-info: enumerate every interface of the battery class.
            if SetupDiEnumDeviceInterfaces(set, ptr::null(), &GUID_DEVICE_BATTERY, index, &mut iface)
                == 0
            {
                break;
            }
            index += 1;

            // First detail call: required buffer size.
            let mut required: u32 = 0;
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &iface,
                ptr::null_mut(),
                0,
                &mut required,
                ptr::null_mut(),
            );
            if required == 0 {
                continue;
            }

            // Second call: u32 buffer so the struct pointer is 4-byte aligned
            // (a Vec<u8> would be 1-aligned → UB writing cbSize). cbSize is the
            // FIXED header size, not the buffer size (same idiom as pcie_link).
            let mut buf = vec![0u32; (required as usize + 3) / 4];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &iface,
                detail,
                required,
                ptr::null_mut(),
                ptr::null_mut(),
            ) == 0
            {
                continue;
            }

            let path_ptr = (*detail).DevicePath.as_ptr();
            let handle = CreateFileW(
                path_ptr,
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            );
            if handle == INVALID_HANDLE_VALUE {
                continue;
            }

            let cycles = query_battery_cycles(handle);
            CloseHandle(handle);

            // First battery that reports a real (non-zero) cycle count wins.
            if let Some(c) = cycles {
                result = Some(c);
                break;
            }
        }

        SetupDiDestroyDeviceInfoList(set);
        result
    }
}

/// Issue the tag + information IOCTLs on an open battery handle.
///
/// # Safety
/// `handle` must be a valid battery device handle opened with read/write access.
unsafe fn query_battery_cycles(handle: HANDLE) -> Option<u32> {
    // 1) Resolve the battery tag (required input to every battery query). The
    //    input is a wait timeout in ms; 0 = don't wait.
    let mut wait: u32 = 0;
    let mut tag: u32 = 0;
    let mut bytes: u32 = 0;
    let ok = DeviceIoControl(
        handle,
        IOCTL_BATTERY_QUERY_TAG,
        &mut wait as *mut u32 as *mut c_void,
        std::mem::size_of::<u32>() as u32,
        &mut tag as *mut u32 as *mut c_void,
        std::mem::size_of::<u32>() as u32,
        &mut bytes,
        ptr::null_mut(),
    );
    // Tag 0 is the invalid/no-battery sentinel — bail before querying info.
    if ok == 0 || tag == 0 {
        return None;
    }

    // 2) Query the BatteryInformation level for this tag.
    let mut query = BatteryQueryInformation {
        battery_tag: tag,
        information_level: BATTERY_INFORMATION_LEVEL,
        at_rate: 0,
    };
    let mut info = [0u8; BATTERY_INFORMATION_SIZE];
    let mut bytes2: u32 = 0;
    let ok2 = DeviceIoControl(
        handle,
        IOCTL_BATTERY_QUERY_INFORMATION,
        &mut query as *mut BatteryQueryInformation as *mut c_void,
        std::mem::size_of::<BatteryQueryInformation>() as u32,
        info.as_mut_ptr() as *mut c_void,
        info.len() as u32,
        &mut bytes2,
        ptr::null_mut(),
    );
    // Validate BEFORE parsing (Eje 7): a short/failed return means don't trust
    // the buffer — degrade to None rather than read uninitialized bytes.
    if ok2 == 0 || (bytes2 as usize) < BATTERY_INFORMATION_SIZE {
        return None;
    }

    parse_battery_information(&info).and_then(|b| b.cycle_count_opt())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 36-byte BATTERY_INFORMATION with a few fields populated.
    fn sample_info(cycle_count: u32) -> [u8; BATTERY_INFORMATION_SIZE] {
        let mut b = [0u8; BATTERY_INFORMATION_SIZE];
        b[12..16].copy_from_slice(&50_000u32.to_le_bytes()); // DesignedCapacity
        b[16..20].copy_from_slice(&46_000u32.to_le_bytes()); // FullChargedCapacity
        b[32..36].copy_from_slice(&cycle_count.to_le_bytes()); // CycleCount
        b
    }

    #[test]
    fn parses_known_fields() {
        let info = parse_battery_information(&sample_info(312)).unwrap();
        assert_eq!(info.designed_capacity, 50_000);
        assert_eq!(info.full_charged_capacity, 46_000);
        assert_eq!(info.cycle_count, 312);
    }

    #[test]
    fn nonzero_cycle_count_is_some() {
        let info = parse_battery_information(&sample_info(312)).unwrap();
        assert_eq!(info.cycle_count_opt(), Some(312));
    }

    #[test]
    fn zero_cycle_count_is_none() {
        // Firmware that doesn't report cycles → 0 → None, never "0 cycles".
        let info = parse_battery_information(&sample_info(0)).unwrap();
        assert_eq!(info.cycle_count_opt(), None);
    }

    #[test]
    fn too_short_buffer_is_none() {
        assert_eq!(parse_battery_information(&[0u8; 16]), None);
    }

    /// Diagnostic — dumps the native cycle count. Run with:
    ///   cargo test --lib battery_ioctl::tests::dump -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_battery_cycle_count() {
        match read_battery_cycle_count() {
            Some(c) => println!("\n=== Battery cycle count (IOCTL): {c} ===\n"),
            None => println!("\n=== Battery cycle count (IOCTL): None (no battery / unsupported) ===\n"),
        }
    }
}
