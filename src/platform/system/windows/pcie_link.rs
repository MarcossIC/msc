//! PCIe link-speed reader.
//!
//! Maps each physical disk to the *current* PCIe generation/width of its
//! controller, so storage info can report the real link (e.g. one NVMe at
//! Gen4 x4, another at Gen4 x2) instead of guessing. Everything is native
//! (SetupAPI + cfgmgr32 + one IOCTL) — no external process, ~1-4ms total.
//!
//! Chain per disk:
//!   1. enumerate present disks (GUID_DEVINTERFACE_DISK)
//!   2. climb the devnode tree (CM_Get_Parent) to the first ancestor exposing
//!      DEVPKEY_PciDevice_CurrentLinkSpeed — that's the PCIe controller
//!   3. open the disk and ask IOCTL_STORAGE_GET_DEVICE_NUMBER for its physical
//!      drive number (the canonical key storage.rs already resolves)
//!   4. map physical_number -> InterfaceSpeed::Pcie { gen, width }

use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;

use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_PropertyW, CM_Get_Parent, SetupDiDestroyDeviceInfoList,
    SetupDiEnumDeviceInfo, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, CR_SUCCESS, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
};
// DEVPROPKEY lives in Foundation (not Devices::Properties) in windows-sys 0.61.
use windows_sys::Win32::Foundation::{CloseHandle, DEVPROPKEY, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use crate::core::system_info::types::InterfaceSpeed;

/// `DEVPROP_TYPE_UINT32` — the property type of the PCIe link values.
const DEVPROP_TYPE_UINT32: u32 = 0x0000_0007;

/// Shared `fmtid` for all `DEVPKEY_PciDevice_*` keys: {3AB22E31-8264-4B4E-9AF5-A8D2D8E33E62}
const PCI_DEVICE_FMTID: GUID = GUID {
    data1: 0x3AB2_2E31,
    data2: 0x8264,
    data3: 0x4B4E,
    data4: [0x9A, 0xF5, 0xA8, 0xD2, 0xD8, 0xE3, 0x3E, 0x62],
};

/// DEVPKEY_PciDevice_CurrentLinkSpeed (pid 9) — enum: 1=Gen1 … 5=Gen5, 6=Gen6.
const DEVPKEY_PCI_CURRENT_LINK_SPEED: DEVPROPKEY = DEVPROPKEY {
    fmtid: PCI_DEVICE_FMTID,
    pid: 9,
};

/// DEVPKEY_PciDevice_CurrentLinkWidth (pid 10) — lane count (1,2,4,8,16).
const DEVPKEY_PCI_CURRENT_LINK_WIDTH: DEVPROPKEY = DEVPROPKEY {
    fmtid: PCI_DEVICE_FMTID,
    pid: 10,
};

/// {53F56307-B6BF-11D0-94F2-00A0C91EFB8B} — GUID_DEVINTERFACE_DISK (local to
/// avoid pulling in the Win32_System_Ioctl feature).
const GUID_DEVINTERFACE_DISK: GUID = GUID::from_u128(0x53f56307_b6bf_11d0_94f2_00a0c91efb8b);

/// IOCTL_STORAGE_GET_DEVICE_NUMBER = CTL_CODE(IOCTL_STORAGE_BASE, 0x0420, METHOD_BUFFERED, FILE_ANY_ACCESS).
const IOCTL_STORAGE_GET_DEVICE_NUMBER: u32 = 0x002D_1080;

/// Layout of STORAGE_DEVICE_NUMBER — we only read `device_number`.
#[repr(C)]
#[allow(dead_code)] // device_type / partition_number are part of the C layout
struct StorageDeviceNumber {
    device_type: u32,
    device_number: u32,
    partition_number: i32,
}

/// Map the CurrentLinkSpeed enum value to a PCIe generation (1–6).
fn link_speed_to_gen(value: u32) -> Option<u8> {
    match value {
        1..=6 => Some(value as u8),
        _ => None,
    }
}

/// Read a UINT32 devnode property via cfgmgr32 (operates on a DEVINST).
///
/// # Safety
/// `devinst` must be a valid DEVINST from this device tree.
unsafe fn read_devnode_u32_prop(devinst: u32, key: *const DEVPROPKEY) -> Option<u32> {
    let mut prop_type: u32 = 0;
    let mut buffer = [0u8; 4];
    // size is in/out for CM_Get_DevNode_PropertyW.
    let mut size: u32 = buffer.len() as u32;
    let cr = CM_Get_DevNode_PropertyW(
        devinst,
        key,
        &mut prop_type,
        buffer.as_mut_ptr(),
        &mut size,
        0,
    );
    if cr == CR_SUCCESS && prop_type == DEVPROP_TYPE_UINT32 {
        Some(u32::from_ne_bytes(buffer))
    } else {
        None
    }
}

/// Climb from `start` until an ancestor exposes a PCIe CurrentLinkSpeed.
/// The immediate parent is sometimes a bridge without the property, so we keep
/// climbing (capped) until we hit the PCIe function. Returns (gen, width).
///
/// # Safety
/// `start` must be a valid DEVINST.
unsafe fn find_pcie_parent_link(start: u32) -> Option<(u8, u8)> {
    let mut current = start;
    for _ in 0..16u32 {
        let mut parent: u32 = 0;
        if CM_Get_Parent(&mut parent, current, 0) != CR_SUCCESS {
            return None;
        }
        if let Some(speed_val) = read_devnode_u32_prop(parent, &DEVPKEY_PCI_CURRENT_LINK_SPEED) {
            let gen = link_speed_to_gen(speed_val)?;
            let width = read_devnode_u32_prop(parent, &DEVPKEY_PCI_CURRENT_LINK_WIDTH)
                .unwrap_or(0) as u8;
            return Some((gen, width));
        }
        current = parent;
    }
    None
}

/// Resolve the physical drive number of an open disk handle via IOCTL.
///
/// # Safety
/// `handle` must be a valid device handle opened on a disk.
unsafe fn query_device_number(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<u32> {
    let mut sdn: StorageDeviceNumber = std::mem::zeroed();
    let mut bytes: u32 = 0;
    let ok = DeviceIoControl(
        handle,
        IOCTL_STORAGE_GET_DEVICE_NUMBER,
        ptr::null(),
        0,
        &mut sdn as *mut StorageDeviceNumber as *mut c_void,
        std::mem::size_of::<StorageDeviceNumber>() as u32,
        &mut bytes,
        ptr::null_mut(),
    );
    if ok != 0 {
        Some(sdn.device_number)
    } else {
        None
    }
}

/// Build a `physical_drive_number -> InterfaceSpeed::Pcie` map for present disks.
///
/// Errors on any single disk are skipped (it simply won't appear in the map, so
/// that disk keeps `interface_speed = None`). Never panics.
pub fn read_disk_link_map() -> HashMap<u32, InterfaceSpeed> {
    let mut map = HashMap::new();

    // SAFETY: SetupAPI interface enumeration; the set and every opened handle
    // are released before returning.
    unsafe {
        let set = SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_DISK,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_DEVICEINTERFACE | DIGCF_PRESENT,
        );
        if set == INVALID_HANDLE_VALUE as isize {
            return map;
        }

        let mut index: u32 = 0;
        loop {
            let mut dev_data: SP_DEVINFO_DATA = std::mem::zeroed();
            dev_data.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiEnumDeviceInfo(set, index, &mut dev_data) == 0 {
                break;
            }
            index += 1;

            // The PCIe link of this disk's controller (skip non-PCIe disks).
            let Some((gen, width)) = find_pcie_parent_link(dev_data.DevInst) else {
                continue;
            };

            // Resolve this disk's device interface path.
            let mut iface: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
            iface.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(set, &dev_data, &GUID_DEVINTERFACE_DISK, 0, &mut iface)
                == 0
            {
                continue;
            }

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

            // Second call: fill the variable-length detail (cbSize is the FIXED
            // header size, not the buffer size). Use a u32 buffer so the struct
            // pointer is 4-byte aligned (Vec<u8> would only be 1-aligned → UB
            // when writing cbSize).
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

            // DevicePath is a NUL-terminated WCHAR string starting at the field.
            let path_ptr = (*detail).DevicePath.as_ptr();
            // Access 0 is enough for IOCTL_STORAGE_GET_DEVICE_NUMBER (no read perms).
            let handle = CreateFileW(
                path_ptr,
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            );
            if handle == INVALID_HANDLE_VALUE {
                continue;
            }

            let number = query_device_number(handle);
            CloseHandle(handle);

            if let Some(n) = number {
                map.insert(n, InterfaceSpeed::Pcie { gen, width });
            }
        }

        SetupDiDestroyDeviceInfoList(set);
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_speed_maps_to_generation() {
        assert_eq!(link_speed_to_gen(3), Some(3));
        assert_eq!(link_speed_to_gen(4), Some(4));
        assert_eq!(link_speed_to_gen(5), Some(5));
        assert_eq!(link_speed_to_gen(0), None);
        assert_eq!(link_speed_to_gen(99), None);
    }

    /// Diagnostic — prints the disk→link map and timing. Run with:
    ///   cargo test --lib pcie_link::tests::dump_disk -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_disk_link_map() {
        use std::time::Instant;
        let start = Instant::now();
        let map = read_disk_link_map();
        let elapsed = start.elapsed();
        println!("\n=== Disk link map ({} entries) in {:?} ===", map.len(), elapsed);
        for (num, speed) in &map {
            println!("  PhysicalDrive{} -> {}", num, speed);
        }
        println!("==============================================\n");
        assert!(elapsed.as_secs() < 10);
    }
}
