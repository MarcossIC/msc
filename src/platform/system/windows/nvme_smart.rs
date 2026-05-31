//! NVMe SMART / Health Information reader (log page 02h).
//!
//! Reads the NVMe SMART/Health Information Log directly via
//! `IOCTL_STORAGE_QUERY_PROPERTY` + `DeviceIoControl` against the in-box
//! stornvme.sys driver. This is the honest source for the data WMI either
//! lacks or mislabels: wear (`PercentageUsed`), host bytes written/read,
//! composite temperature, power-on hours, unsafe shutdowns and media errors.
//!
//! Why IOCTL over WMI `MSFT_StorageReliabilityCounter`:
//!   - Sub-millisecond (one kernel round-trip, 512 bytes) vs ~90ms+/disk.
//!   - Honest host-writes: WMI exposes only `ReadErrorsTotal`/`WriteErrorsTotal`
//!     (error COUNTS, not bytes) — using them as "Data Written" was a bug.
//!   - **No admin**: the handle is opened with `dwDesiredAccess = 0`, which the
//!     CreateFile docs explicitly allow for querying device attributes /
//!     statistics without elevation. (The pass-through
//!     `IOCTL_STORAGE_PROTOCOL_COMMAND` would need admin — we never use it.)
//!
//! Design (per the project's honesty rule + testability):
//!   - [`parse_nvme_health`] is PURE — it reads a 512-byte buffer by offset and
//!     is covered by golden tests, no hardware needed. That is where the bugs
//!     that would fabricate data live (endianness, Kelvin, unit scaling).
//!   - [`read_nvme_health`] is the isolated `unsafe` fetch. ANY failure (RAID /
//!     Intel RST / vendor driver that rejects the IOCTL, no such drive, access
//!     denied) returns `None` — never a fabricated reading.

use std::ffi::c_void;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

// --- IOCTL constants (defined locally to avoid pulling in the
// Win32_System_Ioctl / Win32_Storage_Nvme features, mirroring pcie_link.rs).
// Values verified against windows-sys 0.61.2. ---

/// `IOCTL_STORAGE_QUERY_PROPERTY` = CTL_CODE(IOCTL_STORAGE_BASE=0x2D, 0x0500,
/// METHOD_BUFFERED, FILE_ANY_ACCESS) = 0x002D1400 (2954240).
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
/// `StorageDeviceProtocolSpecificProperty` (STORAGE_PROPERTY_ID).
const STORAGE_DEVICE_PROTOCOL_SPECIFIC_PROPERTY: i32 = 50;
/// `PropertyStandardQuery` (STORAGE_QUERY_TYPE).
const PROPERTY_STANDARD_QUERY: i32 = 0;
/// `ProtocolTypeNvme` (STORAGE_PROTOCOL_TYPE).
const PROTOCOL_TYPE_NVME: i32 = 3;
/// `NVMeDataTypeLogPage` (STORAGE_PROTOCOL_NVME_DATA_TYPE).
const NVME_DATA_TYPE_LOG_PAGE: u32 = 2;
/// `NVME_LOG_PAGE_HEALTH_INFO` — the SMART/Health Information log page (02h).
const NVME_LOG_PAGE_HEALTH_INFO: u32 = 0x02;
/// `sizeof(STORAGE_PROTOCOL_SPECIFIC_DATA)` (10 × u32). The driver places the
/// returned log at `offsetof(descriptor, ProtocolSpecificData) + this`.
const STORAGE_PROTOCOL_SPECIFIC_DATA_SIZE: u32 = 40;
/// Size of the NVMe SMART/Health log page (`NVME_HEALTH_INFO_LOG`).
const NVME_HEALTH_LOG_SIZE: u32 = 512;

/// Combined query/output buffer for the SMART/Health log IOCTL.
///
/// On INPUT this is `STORAGE_PROPERTY_QUERY` (PropertyId, QueryType) whose
/// `AdditionalParameters` (at offset 8) is a `STORAGE_PROTOCOL_SPECIFIC_DATA`,
/// followed by a 512-byte region for the returned log. On OUTPUT the driver
/// reinterprets the head as `STORAGE_PROTOCOL_DATA_DESCRIPTOR` and writes the
/// log at offset `8 + ProtocolDataOffset` (standard drivers: 8 + 40 = 48), i.e.
/// exactly into `data`. Using one `#[repr(C)]` struct keeps it 4-byte aligned
/// and avoids hand-rolled offset math.
#[repr(C)]
// Most fields are written for the FFI input layout and never read back from Rust
// (only `data_offset` and `data` are) — they're part of the C struct contract.
#[allow(dead_code)]
struct NvmeHealthQuery {
    // STORAGE_PROPERTY_QUERY header
    property_id: i32,
    query_type: i32,
    // STORAGE_PROTOCOL_SPECIFIC_DATA (overlays AdditionalParameters @ offset 8)
    protocol_type: i32,
    data_type: u32,
    request_value: u32,
    request_sub_value: u32,
    data_offset: u32,
    data_length: u32,
    fixed_return: u32,
    sub_value2: u32,
    sub_value3: u32,
    sub_value4: u32,
    // Returned log page payload (@ offset 48)
    data: [u8; 512],
}

/// Parsed subset of the NVMe SMART/Health Information Log (log page 02h).
///
/// 128-bit counters are kept as `u128` exactly as the spec defines them; the
/// caller narrows where a smaller type is honest (e.g. power-on hours → u64).
#[derive(Debug, Clone, PartialEq)]
// Several fields (spare, power cycles, unsafe shutdowns, media errors, critical
// warning) are currently surfaced only by the `--ignored` diagnostic dump and
// are reserved for the Etapa 2.4 health panel — kept parsed and honest now.
#[allow(dead_code)]
pub struct NvmeHealth {
    /// Critical Warning bitfield (byte 0): bit0 spare low, bit1 temp threshold,
    /// bit2 reliability degraded, bit3 read-only, bit4 volatile backup failed.
    pub critical_warning: u8,
    /// Composite temperature in °C, or `None` when the reading is the absolute
    /// placeholder (0 K) or outside a plausible range.
    pub composite_temp_c: Option<u32>,
    /// Available spare as a normalized percentage (0–100).
    pub available_spare_pct: u8,
    /// `PercentageUsed` — the honest wear estimate. 100 does NOT mean "dead",
    /// and the value is allowed to exceed 100 (spec caps the reported value at
    /// 255). Render accordingly.
    pub percentage_used: u8,
    /// Data units read (1 unit = 1000 × 512 bytes, per spec).
    pub data_units_read: u128,
    /// Data units written (1 unit = 1000 × 512 bytes, per spec).
    pub data_units_written: u128,
    pub power_cycles: u128,
    pub power_on_hours: u128,
    pub unsafe_shutdowns: u128,
    pub media_errors: u128,
}

impl NvmeHealth {
    /// Host bytes written = data units × 1000 × 512 (NVMe spec). Saturated into
    /// `u64` (18 EB ceiling — far above any real drive's lifetime writes).
    pub fn bytes_written(&self) -> u64 {
        data_units_to_bytes(self.data_units_written)
    }

    /// Host bytes read = data units × 1000 × 512 (NVMe spec).
    pub fn bytes_read(&self) -> u64 {
        data_units_to_bytes(self.data_units_read)
    }

    /// Power-on hours narrowed to `u64` (saturating).
    pub fn power_on_hours_u64(&self) -> u64 {
        self.power_on_hours.min(u64::MAX as u128) as u64
    }
}

/// Convert NVMe "data units" (thousands of 512-byte units) to bytes, saturating.
fn data_units_to_bytes(units: u128) -> u64 {
    units
        .saturating_mul(1000)
        .saturating_mul(512)
        .min(u64::MAX as u128) as u64
}

/// Convert a whole-Kelvin composite temperature to °C, rejecting implausible
/// values (0 K placeholder, or anything outside a sane drive range).
fn kelvin_to_celsius(kelvin: u16) -> Option<u32> {
    if kelvin == 0 {
        return None;
    }
    let celsius = kelvin as i32 - 273; // NVMe reports integer Kelvin.
    if (1..=120).contains(&celsius) {
        Some(celsius as u32)
    } else {
        None
    }
}

/// Read a little-endian `u128` from `buf` at `offset` (offset+16 must be in range).
fn read_u128_le(buf: &[u8], offset: usize) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&buf[offset..offset + 16]);
    u128::from_le_bytes(bytes)
}

/// Parse a 512-byte NVMe SMART/Health Information Log (log page 02h).
///
/// Pure: no I/O, no `unsafe`. Returns `None` if the buffer is too short. Field
/// offsets follow `NVME_HEALTH_INFO_LOG` (Microsoft Learn / windows-sys):
///   0    CriticalWarning (u8)        1..3  Temperature (u16 LE, Kelvin)
///   3    AvailableSpare (u8)         5     PercentageUsed (u8)
///   32   DataUnitRead (u128 LE)      48    DataUnitWritten (u128 LE)
///   112  PowerCycle (u128 LE)        128   PowerOnHours (u128 LE)
///   144  UnsafeShutdowns (u128 LE)   160   MediaErrors (u128 LE)
pub fn parse_nvme_health(buf: &[u8]) -> Option<NvmeHealth> {
    if buf.len() < NVME_HEALTH_LOG_SIZE as usize {
        return None;
    }

    let temp_kelvin = u16::from_le_bytes([buf[1], buf[2]]);

    Some(NvmeHealth {
        critical_warning: buf[0],
        composite_temp_c: kelvin_to_celsius(temp_kelvin),
        available_spare_pct: buf[3],
        percentage_used: buf[5],
        data_units_read: read_u128_le(buf, 32),
        data_units_written: read_u128_le(buf, 48),
        power_cycles: read_u128_le(buf, 112),
        power_on_hours: read_u128_le(buf, 128),
        unsafe_shutdowns: read_u128_le(buf, 144),
        media_errors: read_u128_le(buf, 160),
    })
}

/// Read the NVMe SMART/Health log for `\\.\PhysicalDrive{physical_drive}`.
///
/// Returns `None` on ANY failure — handle can't be opened, the IOCTL is
/// unsupported (RAID/RST/vendor driver), or the descriptor came back
/// nonstandard. Never fabricates a reading. Safe wrapper around the isolated
/// `unsafe` block.
pub fn read_nvme_health(physical_drive: u32) -> Option<NvmeHealth> {
    // \\.\PhysicalDriveN as a NUL-terminated wide string.
    let path: Vec<u16> = format!("\\\\.\\PhysicalDrive{}", physical_drive)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: `path` is a valid NUL-terminated UTF-16 buffer. `dwDesiredAccess = 0`
    // opens the device only to query attributes (no admin needed, per CreateFile
    // docs). The handle is closed before returning on every path.
    unsafe {
        let handle = CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        let result = query_health_log(handle);
        CloseHandle(handle);
        result
    }
}

/// Issue the SMART/Health log IOCTL on an open disk handle and parse the result.
///
/// # Safety
/// `handle` must be a valid device handle opened on a physical disk.
unsafe fn query_health_log(handle: HANDLE) -> Option<NvmeHealth> {
    // Zero-initialized so that even an unexpected success never reads
    // uninitialized memory.
    let mut q: NvmeHealthQuery = std::mem::zeroed();
    q.property_id = STORAGE_DEVICE_PROTOCOL_SPECIFIC_PROPERTY;
    q.query_type = PROPERTY_STANDARD_QUERY;
    q.protocol_type = PROTOCOL_TYPE_NVME;
    q.data_type = NVME_DATA_TYPE_LOG_PAGE;
    q.request_value = NVME_LOG_PAGE_HEALTH_INFO;
    q.request_sub_value = 0;
    q.data_offset = STORAGE_PROTOCOL_SPECIFIC_DATA_SIZE;
    q.data_length = NVME_HEALTH_LOG_SIZE;

    let size = std::mem::size_of::<NvmeHealthQuery>() as u32;
    let mut returned: u32 = 0;

    let ok = DeviceIoControl(
        handle,
        IOCTL_STORAGE_QUERY_PROPERTY,
        &mut q as *mut NvmeHealthQuery as *mut c_void,
        size,
        &mut q as *mut NvmeHealthQuery as *mut c_void,
        size,
        &mut returned,
        ptr::null_mut(),
    );

    // Validate BEFORE interpreting the buffer (Eje 7): a failed IOCTL means the
    // driver doesn't support this path — degrade to None, don't read garbage.
    if ok == 0 || returned == 0 {
        return None;
    }

    // On return, `data_offset` holds the descriptor's ProtocolDataOffset. Standard
    // drivers report 40, which places the log exactly in `q.data`. Anything else
    // is nonstandard layout — bail rather than read the wrong bytes.
    if q.data_offset != STORAGE_PROTOCOL_SPECIFIC_DATA_SIZE {
        return None;
    }

    parse_nvme_health(&q.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 512-byte health log with a few fields populated for golden tests.
    fn sample_log() -> [u8; 512] {
        let mut b = [0u8; 512];
        b[0] = 0x01; // CriticalWarning: spare low
                     // Temperature: 313 K (= 40 °C) → 0x0139 little-endian.
        b[1] = 0x39;
        b[2] = 0x01;
        b[3] = 95; // AvailableSpare 95%
        b[5] = 7; // PercentageUsed (wear) 7%
                  // DataUnitWritten @48 = 1_000_000 units.
        b[48..64].copy_from_slice(&1_000_000u128.to_le_bytes());
        // DataUnitRead @32 = 2_000_000 units.
        b[32..48].copy_from_slice(&2_000_000u128.to_le_bytes());
        // PowerOnHours @128 = 5000.
        b[128..144].copy_from_slice(&5000u128.to_le_bytes());
        b
    }

    #[test]
    fn parses_known_fields() {
        let h = parse_nvme_health(&sample_log()).unwrap();
        assert_eq!(h.critical_warning, 0x01);
        assert_eq!(h.composite_temp_c, Some(40));
        assert_eq!(h.available_spare_pct, 95);
        assert_eq!(h.percentage_used, 7);
        assert_eq!(h.data_units_written, 1_000_000);
        assert_eq!(h.data_units_read, 2_000_000);
        assert_eq!(h.power_on_hours, 5000);
    }

    #[test]
    fn data_units_convert_to_bytes_per_spec() {
        let h = parse_nvme_health(&sample_log()).unwrap();
        // 1_000_000 units × 1000 × 512 = 512_000_000_000 bytes (512 GB).
        assert_eq!(h.bytes_written(), 512_000_000_000);
        assert_eq!(h.bytes_read(), 1_024_000_000_000);
    }

    #[test]
    fn power_on_hours_narrow_to_u64() {
        let h = parse_nvme_health(&sample_log()).unwrap();
        assert_eq!(h.power_on_hours_u64(), 5000);
    }

    #[test]
    fn too_short_buffer_is_none() {
        assert_eq!(parse_nvme_health(&[0u8; 256]), None);
    }

    #[test]
    fn zero_kelvin_temperature_is_none() {
        // All-zero log: temperature 0 K is the placeholder, not 0 °C.
        let h = parse_nvme_health(&[0u8; 512]).unwrap();
        assert_eq!(h.composite_temp_c, None);
        assert_eq!(h.percentage_used, 0);
    }

    #[test]
    fn implausible_temperature_is_rejected() {
        // 1000 K → 727 °C, absurd → None (never shown as a real reading).
        assert_eq!(kelvin_to_celsius(1000), None);
        // 273 K = 0 °C, the classic "unsupported" sentinel → None.
        assert_eq!(kelvin_to_celsius(273), None);
        // 313 K = 40 °C, plausible.
        assert_eq!(kelvin_to_celsius(313), Some(40));
    }

    #[test]
    fn data_units_to_bytes_saturates() {
        // u128::MAX units must not panic/overflow — saturates to u64::MAX.
        assert_eq!(data_units_to_bytes(u128::MAX), u64::MAX);
    }

    /// Diagnostic — dumps real health for each physical drive. Run with:
    ///   cargo test --lib nvme_smart::tests::dump -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_nvme_health() {
        for n in 0..8u32 {
            if let Some(h) = read_nvme_health(n) {
                println!("\n=== PhysicalDrive{n} ===");
                println!("  wear:        {}%", h.percentage_used);
                println!("  spare:       {}%", h.available_spare_pct);
                println!("  temp:        {:?} °C", h.composite_temp_c);
                println!("  written:     {} bytes", h.bytes_written());
                println!("  read:        {} bytes", h.bytes_read());
                println!("  power-on:    {} h", h.power_on_hours_u64());
                println!("  power cycle: {}", h.power_cycles);
                println!("  unsafe shut: {}", h.unsafe_shutdowns);
                println!("  media err:   {}", h.media_errors);
                println!("  crit warn:   {:#04x}", h.critical_warning);
            }
        }
    }
}
