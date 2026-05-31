//! Wi-Fi SSID reader via the Native Wifi API (wlanapi).
//!
//! Resolves the SSID of the currently associated wireless interface without
//! spawning `netsh` — everything is native (`WlanOpenHandle` + `WlanEnumInterfaces`
//! + `WlanQueryInterface`), mirroring the spawn-free approach of `pcie_link.rs`.
//! The `Win32_NetworkManagement_WiFi` feature is already enabled in `Cargo.toml`.
//!
//! Chain:
//!   1. open a WLAN client handle (API v2)
//!   2. enumerate interfaces; for the first one in `connected` state
//!   3. query `current_connection` and read `dot11Ssid` from the association
//!      attributes
//!   4. decode the SSID bytes (UTF-8, lossy) into a `String`

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows_sys::Win32::NetworkManagement::WiFi::{
    wlan_intf_opcode_current_connection, wlan_interface_state_connected, WlanCloseHandle,
    WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle, WlanQueryInterface,
    WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
};

/// Native Wifi client API version requested (v2 — Vista+).
#[cfg(windows)]
const WLAN_API_VERSION_2: u32 = 2;

/// Decode a `DOT11_SSID` (length + raw bytes) into a printable name.
///
/// SSIDs are at most 32 bytes and are *not* guaranteed valid UTF-8, so we decode
/// lossily and drop empty/whitespace-only results (a hidden or unset SSID).
fn parse_ssid(len: u32, raw: &[u8]) -> Option<String> {
    let len = (len as usize).min(raw.len());
    if len == 0 {
        return None;
    }
    let name = String::from_utf8_lossy(&raw[..len]).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// SSID of the currently connected Wi-Fi interface, or `None` if not associated
/// / no WLAN service. Best-effort and never panics; any API failure → `None`.
#[cfg(windows)]
pub fn get_connected_ssid() -> Option<String> {
    // SAFETY: every successful WLAN allocation/handle is released before return:
    // the interface list and the queried connection attributes via
    // `WlanFreeMemory`, the client handle via `WlanCloseHandle`.
    unsafe {
        let mut handle: HANDLE = ptr::null_mut();
        let mut negotiated: u32 = 0;
        if WlanOpenHandle(
            WLAN_API_VERSION_2,
            ptr::null(),
            &mut negotiated,
            &mut handle,
        ) != 0
        {
            return None;
        }

        let ssid = enum_and_query_ssid(handle);

        WlanCloseHandle(handle, ptr::null());
        ssid
    }
}

/// Enumerate interfaces and return the SSID of the first connected one.
///
/// # Safety
/// `handle` must be a live client handle from `WlanOpenHandle`.
#[cfg(windows)]
unsafe fn enum_and_query_ssid(handle: HANDLE) -> Option<String> {
    let mut list_ptr: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
    if WlanEnumInterfaces(handle, ptr::null(), &mut list_ptr) != 0 || list_ptr.is_null() {
        return None;
    }

    let mut result = None;
    let count = (*list_ptr).dwNumberOfItems as usize;
    let infos = (*list_ptr).InterfaceInfo.as_ptr();

    for i in 0..count {
        let info = &*infos.add(i);
        if info.isState != wlan_interface_state_connected {
            continue;
        }

        if let Some(ssid) = query_interface_ssid(handle, &info.InterfaceGuid) {
            result = Some(ssid);
            break;
        }
    }

    WlanFreeMemory(list_ptr as *const c_void);
    result
}

/// Query `current_connection` for one interface GUID and extract its SSID.
///
/// # Safety
/// `handle` must be live and `guid` must point to a valid interface GUID from
/// the enumeration above.
#[cfg(windows)]
unsafe fn query_interface_ssid(
    handle: HANDLE,
    guid: *const windows_sys::core::GUID,
) -> Option<String> {
    let mut data_size: u32 = 0;
    let mut data_ptr: *mut c_void = ptr::null_mut();

    if WlanQueryInterface(
        handle,
        guid,
        wlan_intf_opcode_current_connection,
        ptr::null(),
        &mut data_size,
        &mut data_ptr,
        ptr::null_mut(),
    ) != 0
        || data_ptr.is_null()
        || (data_size as usize) < std::mem::size_of::<WLAN_CONNECTION_ATTRIBUTES>()
    {
        return None;
    }

    let attrs = &*(data_ptr as *const WLAN_CONNECTION_ATTRIBUTES);
    let dot11 = &attrs.wlanAssociationAttributes.dot11Ssid;
    let ssid = parse_ssid(dot11.uSSIDLength, &dot11.ucSSID);

    WlanFreeMemory(data_ptr as *const c_void);
    ssid
}

#[cfg(not(windows))]
pub fn get_connected_ssid() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::parse_ssid;

    #[test]
    fn decodes_ascii_ssid() {
        let mut raw = [0u8; 32];
        raw[..7].copy_from_slice(b"MyWiFi5");
        assert_eq!(parse_ssid(7, &raw), Some("MyWiFi5".to_string()));
    }

    #[test]
    fn ignores_bytes_past_declared_length() {
        let mut raw = [0u8; 32];
        raw[..4].copy_from_slice(b"Home");
        raw[4] = b'X'; // garbage beyond uSSIDLength must be ignored
        assert_eq!(parse_ssid(4, &raw), Some("Home".to_string()));
    }

    #[test]
    fn empty_or_blank_ssid_is_none() {
        assert_eq!(parse_ssid(0, &[0u8; 32]), None);
        let blank = [b' '; 32];
        assert_eq!(parse_ssid(5, &blank), None);
    }

    #[test]
    fn clamps_length_to_buffer() {
        let raw = *b"AP";
        // A bogus length larger than the slice must not panic.
        assert_eq!(parse_ssid(99, &raw), Some("AP".to_string()));
    }
}
