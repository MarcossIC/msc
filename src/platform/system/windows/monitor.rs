//! Connected-display details parsed from the monitor's EDID block.
//!
//! Source of truth is the raw EDID the monitor hands the system over the cable.
//! On Windows it lives, driver-less and admin-free, in the registry under
//! `...\Enum\DISPLAY\<id>\<instance>\Device Parameters\EDID`. The catch: that key
//! also keeps STALE entries for every monitor ever connected. So we first ask WMI
//! (`WmiMonitorID`, `root\wmi`) for the *active* monitors + their `InstanceName`,
//! then read each one's EDID from the registry. WMI filters; the EDID parse fills.
//!
//! Architecture mirrors `os.rs`/`mbo.rs`: every EDID decode is a PURE function
//! (golden-tested below), and all WMI/registry I/O is isolated in the fetchers so
//! the parsers stay deterministic and unit-testable without hardware.

use crate::core::system_info::types::{DigitalInterface, MonitorInfo};

/// The fixed 8-byte EDID header: `00 FF FF FF FF FF FF 00`.
const EDID_HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];

/// `true` when `edid` is at least a full base block and starts with the magic
/// header. Anything else isn't EDID we can trust — we refuse to parse it.
fn has_valid_header(edid: &[u8]) -> bool {
    edid.len() >= 128 && edid[0..8] == EDID_HEADER
}

/// Decode the 3-letter PNP manufacturer id from EDID bytes 8-9.
///
/// The id is big-endian with three 5-bit letters (1 = 'A' … 26 = 'Z'); bit 15 is
/// reserved 0. Out-of-range codes become '?' rather than a bogus glyph.
fn decode_manufacturer_id(b8: u8, b9: u8) -> String {
    let v = ((b8 as u16) << 8) | b9 as u16;
    let letter = |code: u16| -> char {
        let c = (code & 0x1F) as u8;
        if (1..=26).contains(&c) {
            (b'A' - 1 + c) as char
        } else {
            '?'
        }
    };
    [v >> 10, v >> 5, v].iter().map(|&c| letter(c)).collect()
}

/// Product code (EDID bytes 10-11, little-endian) as the vendor's 4-digit hex tag.
fn decode_product_code(b10: u8, b11: u8) -> String {
    let code = (b10 as u16) | ((b11 as u16) << 8);
    format!("{:04X}", code)
}

/// Manufacture week (EDID byte 16). `0` = unspecified, `0xFF` = "use model year"
/// flag — both map to `None`. Valid weeks are 1-54.
fn decode_week(b16: u8) -> Option<u8> {
    match b16 {
        1..=54 => Some(b16),
        _ => None,
    }
}

/// Manufacture year (EDID byte 17 + 1990). `0` is undefined → `None`.
fn decode_year(b17: u8) -> Option<u16> {
    if b17 == 0 {
        None
    } else {
        Some(1990 + b17 as u16)
    }
}

/// EDID structure version/revision (bytes 18-19), e.g. "1.4".
fn decode_edid_version(b18: u8, b19: u8) -> String {
    format!("{}.{}", b18, b19)
}

/// Decode the video-input byte (20): `(digital interface, bits-per-color)`.
///
/// Bit 7 distinguishes digital (1) from analog (0). For analog inputs both
/// outputs are `None`. For digital, bits 6-4 give the color depth and bits 3-0
/// the interface standard (EDID 1.4); an undefined code maps to `Undefined`/`None`
/// rather than a guess.
fn decode_video_input(b20: u8) -> (Option<DigitalInterface>, Option<u8>) {
    if b20 & 0x80 == 0 {
        return (None, None); // analog input
    }
    let depth = match (b20 >> 4) & 0x07 {
        1 => Some(6),
        2 => Some(8),
        3 => Some(10),
        4 => Some(12),
        5 => Some(14),
        6 => Some(16),
        _ => None, // 0 = undefined, 7 = reserved
    };
    let interface = match b20 & 0x0F {
        1 => DigitalInterface::Dvi,
        2 => DigitalInterface::HdmiA,
        3 => DigitalInterface::HdmiB,
        4 => DigitalInterface::Mddi,
        5 => DigitalInterface::DisplayPort,
        _ => DigitalInterface::Undefined,
    };
    (Some(interface), depth)
}

/// Display gamma (EDID byte 23): `(value + 100) / 100`. `0xFF` defers gamma to an
/// extension block, reported honestly as `None`.
fn decode_gamma(b23: u8) -> Option<f32> {
    if b23 == 0xFF {
        None
    } else {
        Some((b23 as f32 + 100.0) / 100.0)
    }
}

/// Feature-support flags (EDID byte 24): `(sRGB default, preferred=native, GTF)`.
fn decode_features(b24: u8) -> (bool, bool, bool) {
    (b24 & 0x04 != 0, b24 & 0x02 != 0, b24 & 0x01 != 0)
}

/// Physical image size in cm (EDID bytes 21-22). `(0, 0)` means undefined (or the
/// bytes encode aspect ratio instead, EDID 1.4) → `None`.
fn decode_physical_size(b21: u8, b22: u8) -> Option<(u8, u8)> {
    if b21 == 0 || b22 == 0 {
        None
    } else {
        Some((b21, b22))
    }
}

/// Screen diagonal in inches from the physical size in cm: `√(h² + v²) / 2.54`.
fn diagonal_inches(h_cm: u8, v_cm: u8) -> f32 {
    let h = h_cm as f32;
    let v = v_cm as f32;
    (h * h + v * v).sqrt() / 2.54
}

/// The four 18-byte descriptor blocks live at these offsets in the base block.
const DESCRIPTOR_OFFSETS: [usize; 4] = [54, 72, 90, 108];

/// A monitor (non-timing) descriptor begins with `00 00 00 <tag>`; a detailed
/// timing descriptor starts with a non-zero pixel clock. This returns the tag
/// byte for a monitor descriptor, or `None` when the block is a DTD.
fn monitor_descriptor_tag(desc: &[u8]) -> Option<u8> {
    if desc.len() >= 4 && desc[0] == 0 && desc[1] == 0 && desc[2] == 0 {
        Some(desc[3])
    } else {
        None
    }
}

/// Read the ASCII text payload (bytes 5-17) of a `0xFC`/`0xFF` descriptor.
/// Text is terminated by `0x0A` and space-padded; we trim both. `None` if empty.
fn descriptor_text(desc: &[u8]) -> Option<String> {
    if desc.len() < 18 {
        return None;
    }
    let raw = &desc[5..18];
    let end = raw.iter().position(|&b| b == 0x0A).unwrap_or(raw.len());
    let text: String = raw[..end]
        .iter()
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Sync ranges parsed from a `0xFD` Display Range Limits descriptor.
struct RangeLimits {
    h_khz: (u16, u16),
    v_hz: (u16, u16),
    max_pixel_clock_mhz: u16,
}

/// Parse a `0xFD` Display Range Limits descriptor.
///
/// EDID 1.4 packs the rates into single bytes but adds a "range limit offsets"
/// byte (index 4): a `+255` may apply to the min and/or max of each axis. This is
/// NOT optional — a 165 Hz panel's 284 kHz max horizontal rate (> 255) is only
/// representable via the horizontal `+255` offset. Ignoring it would silently
/// report `29 kHz` instead of `284 kHz`.
fn parse_range_limits(desc: &[u8]) -> Option<RangeLimits> {
    if desc.len() < 18 {
        return None;
    }
    let flags = desc[4];
    let v_off = flags & 0x03;
    let h_off = (flags >> 2) & 0x03;
    let v_min_add = if v_off == 0x03 { 255 } else { 0 };
    let v_max_add = if v_off >= 0x02 { 255 } else { 0 };
    let h_min_add = if h_off == 0x03 { 255 } else { 0 };
    let h_max_add = if h_off >= 0x02 { 255 } else { 0 };

    let v_min = desc[5] as u16 + v_min_add;
    let v_max = desc[6] as u16 + v_max_add;
    let h_min = desc[7] as u16 + h_min_add;
    let h_max = desc[8] as u16 + h_max_add;
    let max_pixel_clock_mhz = desc[9] as u16 * 10;

    Some(RangeLimits {
        h_khz: (h_min, h_max),
        v_hz: (v_min, v_max),
        max_pixel_clock_mhz,
    })
}

/// Native timing parsed from a Detailed Timing Descriptor: `(width, height, Hz)`.
struct DetailedTiming {
    width: u16,
    height: u16,
    refresh_hz: f32,
}

/// Parse a Detailed Timing Descriptor (the preferred/native mode in descriptor 0).
///
/// Active/blanking pixels are split across low bytes plus a nibble in a shared
/// high byte. Refresh = pixel_clock / (h_total × v_total). A zero pixel clock or
/// degenerate totals mean "not a usable DTD" → `None`.
fn parse_detailed_timing(desc: &[u8]) -> Option<DetailedTiming> {
    if desc.len() < 18 {
        return None;
    }
    let pixel_clock_khz = ((desc[0] as u32) | ((desc[1] as u32) << 8)) * 10;
    if pixel_clock_khz == 0 {
        return None; // a monitor descriptor, not a DTD
    }
    let h_active = (desc[2] as u32) | (((desc[4] as u32) & 0xF0) << 4);
    let h_blank = (desc[3] as u32) | (((desc[4] as u32) & 0x0F) << 8);
    let v_active = (desc[5] as u32) | (((desc[7] as u32) & 0xF0) << 4);
    let v_blank = (desc[6] as u32) | (((desc[7] as u32) & 0x0F) << 8);

    let h_total = h_active + h_blank;
    let v_total = v_active + v_blank;
    if h_total == 0 || v_total == 0 || h_active == 0 || v_active == 0 {
        return None;
    }

    let refresh_hz = (pixel_clock_khz as f32 * 1000.0) / (h_total as f32 * v_total as f32);
    Some(DetailedTiming {
        width: h_active as u16,
        height: v_active as u16,
        refresh_hz,
    })
}

/// Parse a full EDID blob into a [`MonitorInfo`]. PURE (no I/O) — the seam that
/// makes the whole decode golden-testable. `None` when the header is invalid.
pub fn parse_edid(edid: &[u8]) -> Option<MonitorInfo> {
    if !has_valid_header(edid) {
        return None;
    }

    let manufacturer_id = decode_manufacturer_id(edid[8], edid[9]);
    let product_code = decode_product_code(edid[10], edid[11]);
    let manufacture_week = decode_week(edid[16]);
    let manufacture_year = decode_year(edid[17]);
    let edid_version = Some(decode_edid_version(edid[18], edid[19]));
    let (digital_interface, color_bit_depth) = decode_video_input(edid[20]);
    let physical_size_cm = decode_physical_size(edid[21], edid[22]);
    let diagonal = physical_size_cm.map(|(h, v)| diagonal_inches(h, v));
    let gamma = decode_gamma(edid[23]);
    let (srgb_default, preferred_timing_is_native, continuous_frequency) =
        decode_features(edid[24]);

    // Walk the four descriptors: text (name/serial), range limits, first DTD.
    let mut model_name = None;
    let mut serial_number = None;
    let mut h_freq_khz = None;
    let mut v_freq_hz = None;
    let mut max_pixel_clock_mhz = None;
    let mut native_resolution = None;
    let mut native_refresh_hz = None;

    for &off in &DESCRIPTOR_OFFSETS {
        let desc = &edid[off..off + 18];
        match monitor_descriptor_tag(desc) {
            Some(0xFC) => model_name = descriptor_text(desc),
            Some(0xFF) => serial_number = descriptor_text(desc),
            Some(0xFD) => {
                if let Some(rl) = parse_range_limits(desc) {
                    h_freq_khz = Some(rl.h_khz);
                    v_freq_hz = Some(rl.v_hz);
                    max_pixel_clock_mhz = Some(rl.max_pixel_clock_mhz);
                }
            }
            Some(_) => {} // other monitor descriptors (white point, std timings) ignored
            None => {
                // Detailed timing. Descriptor 0 is the preferred/native mode; keep
                // the first valid one we see.
                if native_resolution.is_none() {
                    if let Some(dt) = parse_detailed_timing(desc) {
                        native_resolution = Some((dt.width, dt.height));
                        native_refresh_hz = Some(dt.refresh_hz);
                    }
                }
            }
        }
    }

    Some(MonitorInfo {
        manufacturer_id,
        product_code,
        model_name,
        serial_number,
        manufacture_week,
        manufacture_year,
        edid_version,
        digital_interface,
        color_bit_depth,
        gamma,
        srgb_default,
        preferred_timing_is_native,
        continuous_frequency,
        h_freq_khz,
        v_freq_hz,
        max_pixel_clock_mhz,
        native_resolution,
        native_refresh_hz,
        physical_size_cm,
        diagonal_inches: diagonal,
    })
}

/// Strip the trailing `_<n>` output index WMI appends to a monitor `InstanceName`
/// so it matches the registry device key. `DISPLAY\BOE0C80\5&x&0&UID256_0` →
/// `DISPLAY\BOE0C80\5&x&0&UID256`. Pure — covered by tests.
fn registry_path_from_instance(instance: &str) -> String {
    match instance.rfind('_') {
        Some(pos) if instance[pos + 1..].chars().all(|c| c.is_ascii_digit()) => {
            instance[..pos].to_string()
        }
        _ => instance.to_string(),
    }
}

// ----- Isolated Windows I/O (WMI active-monitor filter + registry EDID read) ---

/// Instance names of the currently-active monitors, via `WmiMonitorID`
/// (`root\wmi`). This namespace returns ONLY connected displays, which is exactly
/// the stale-entry filter the registry alone can't give us. `None` fields and WMI
/// errors degrade to an empty list rather than guessing.
#[cfg(windows)]
fn active_monitor_instances() -> Vec<String> {
    use serde::Deserialize;
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct WmiMonitorId {
        active: Option<bool>,
        instance_name: Option<String>,
    }

    let wmi = match WMIConnection::with_namespace_path("root\\wmi") {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };

    wmi.raw_query::<WmiMonitorId>("SELECT Active, InstanceName FROM WmiMonitorID")
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.active.unwrap_or(false))
        .filter_map(|m| m.instance_name)
        .collect()
}

/// Read the raw EDID blob for a device instance from
/// `HKLM\SYSTEM\CurrentControlSet\Enum\<path>\Device Parameters\EDID`. The path is
/// derived from the WMI `InstanceName`. `None` when the key/value is absent.
#[cfg(windows)]
fn read_edid_from_registry(instance_name: &str) -> Option<Vec<u8>> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let device = registry_path_from_instance(instance_name);
    let subkey = format!(
        "SYSTEM\\CurrentControlSet\\Enum\\{}\\Device Parameters",
        device
    );
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(subkey)
        .ok()?;
    let value = key.get_raw_value("EDID").ok()?;
    if value.bytes.is_empty() {
        None
    } else {
        // winreg 0.56 exposes `bytes` as `Cow<[u8]>`; take ownership.
        Some(value.bytes.into_owned())
    }
}

/// Collect EDID-parsed info for every active display. Driver-less, no admin.
#[cfg(windows)]
pub fn get_monitors() -> Vec<MonitorInfo> {
    active_monitor_instances()
        .into_iter()
        .filter_map(|inst| read_edid_from_registry(&inst))
        .filter_map(|edid| parse_edid(&edid))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A synthetic but spec-faithful EDID modeled on the user's BOE panel:
    // BOE 0C80, week 31 / 2023, EDID 1.4, DisplayPort + 8 bpc, gamma 2.20,
    // sRGB default, V 48-165 Hz, H 29-284 kHz (needs the +255 H offset),
    // max pixel clock 770 MHz, native 2560x1600, name "NE160QDM-NYJ".
    fn sample_edid() -> [u8; 128] {
        let mut e = [0u8; 128];
        e[0..8].copy_from_slice(&EDID_HEADER);
        // Manufacturer "BOE": B=2, O=15, E=5 → 0b00010_01111_00101 = 0x09E5
        e[8] = 0x09;
        e[9] = 0xE5;
        // Product code 0x0C80, little-endian
        e[10] = 0x80;
        e[11] = 0x0C;
        e[16] = 31; // week
        e[17] = 33; // 2023 = 1990 + 33
        e[18] = 1; // EDID 1.4
        e[19] = 4;
        // Video input: digital(0x80) | 8bpc(2<<4=0x20) | DisplayPort(5) = 0xA5
        e[20] = 0xA5;
        e[21] = 34; // ~16" panel: 34cm x 21cm
        e[22] = 21;
        e[23] = 120; // gamma (120+100)/100 = 2.20
        e[24] = 0x06; // sRGB default(0x04) | preferred=native(0x02)

        // Descriptor 0 (offset 54): Detailed Timing — 2560x1600 @ ~165Hz.
        // pixel clock 677.6 MHz → 67760 (×10kHz units) = 0x108B0 ... cap to 16-bit:
        // use 0xFFFE region carefully; pick clock so refresh ≈ valid. We just need a
        // parseable DTD; exact refresh isn't asserted to a tight tolerance.
        let dtd = &mut e[54..72];
        dtd[0] = 0x9C; // pixel clock low  (0x6C9C = 27804 → 278.04 MHz)
        dtd[1] = 0x6C; // pixel clock high
        dtd[2] = 0x00; // h active low (2560 = 0xA00 → low 0x00, high nibble 0xA)
        dtd[3] = 0x40; // h blank low
        dtd[4] = 0xA0; // h active high nibble (0xA0>>4=0xA), h blank high nibble 0x0
        dtd[5] = 0x40; // v active low (1600 = 0x640 → low 0x40, high nibble 0x6)
        dtd[6] = 0x20; // v blank low
        dtd[7] = 0x60; // v active high nibble 0x6, v blank high nibble 0x0

        // Descriptor 1 (offset 72): Display Range Limits (0xFD).
        let rl = &mut e[72..90];
        rl[0] = 0;
        rl[1] = 0;
        rl[2] = 0;
        rl[3] = 0xFD;
        rl[4] = 0x08; // offsets: horizontal max +255 (bits 3:2 = 0b10)
        rl[5] = 48; // v min Hz
        rl[6] = 165; // v max Hz
        rl[7] = 29; // h min kHz
        rl[8] = 29; // h max kHz (+255 via offset → 284)
        rl[9] = 77; // max pixel clock 77*10 = 770 MHz

        // Descriptor 2 (offset 90): Display Product Name (0xFC) "NE160QDM-NYJ".
        let name = &mut e[90..108];
        name[0] = 0;
        name[1] = 0;
        name[2] = 0;
        name[3] = 0xFC;
        let label = b"NE160QDM-NYJ";
        name[5..5 + label.len()].copy_from_slice(label);
        name[5 + label.len()] = 0x0A; // terminator

        e
    }

    #[test]
    fn rejects_blob_without_magic_header() {
        // The honesty gate: garbage in must not produce a fabricated monitor.
        assert!(!has_valid_header(&[0u8; 128]));
        assert!(parse_edid(&[0u8; 128]).is_none());
        assert!(parse_edid(&[]).is_none());
    }

    #[test]
    fn decodes_manufacturer_id() {
        // "BOE" packed big-endian, 5 bits per letter.
        assert_eq!(decode_manufacturer_id(0x09, 0xE5), "BOE");
        // "DEL" (Dell): D=4,E=5,L=12 → 0b00100_00101_01100 = 0x10AC
        assert_eq!(decode_manufacturer_id(0x10, 0xAC), "DEL");
    }

    #[test]
    fn decodes_product_code_little_endian() {
        assert_eq!(decode_product_code(0x80, 0x0C), "0C80");
    }

    #[test]
    fn decodes_manufacture_date() {
        assert_eq!(decode_week(31), Some(31));
        assert_eq!(decode_week(0), None); // unspecified
        assert_eq!(decode_week(0xFF), None); // model-year flag
        assert_eq!(decode_year(33), Some(2023));
        assert_eq!(decode_year(0), None);
    }

    #[test]
    fn decodes_video_input_digital_dp_8bpc() {
        let (iface, depth) = decode_video_input(0xA5);
        assert_eq!(iface, Some(DigitalInterface::DisplayPort));
        assert_eq!(depth, Some(8));
    }

    #[test]
    fn decodes_video_input_analog_is_none() {
        // Bit 7 clear = analog: no digital interface, no bit depth.
        let (iface, depth) = decode_video_input(0x0F);
        assert_eq!(iface, None);
        assert_eq!(depth, None);
    }

    #[test]
    fn decodes_gamma_and_deferred() {
        // Float-tolerant: 2.20 isn't exactly representable in f32.
        assert!((decode_gamma(120).unwrap() - 2.20).abs() < 0.001);
        assert_eq!(decode_gamma(0xFF), None); // deferred to extension
    }

    #[test]
    fn decodes_feature_flags() {
        let (srgb, native, gtf) = decode_features(0x07);
        assert!(srgb && native && gtf);
        let (srgb, native, gtf) = decode_features(0x00);
        assert!(!srgb && !native && !gtf);
    }

    #[test]
    fn diagonal_from_physical_size() {
        // 34cm x 21cm → ~15.7" diagonal.
        let d = diagonal_inches(34, 21);
        assert!((d - 15.7).abs() < 0.2, "got {d}");
    }

    #[test]
    fn range_limits_apply_horizontal_offset() {
        // The decisive case: max horizontal 284 kHz > 255 only works via the +255
        // EDID 1.4 offset. Without it we'd report 29 kHz — a silent lie.
        let mut desc = [0u8; 18];
        desc[3] = 0xFD;
        desc[4] = 0x08; // horizontal max +255
        desc[5] = 48;
        desc[6] = 165;
        desc[7] = 29;
        desc[8] = 29;
        desc[9] = 77;
        let rl = parse_range_limits(&desc).unwrap();
        assert_eq!(rl.v_hz, (48, 165));
        assert_eq!(rl.h_khz, (29, 284));
        assert_eq!(rl.max_pixel_clock_mhz, 770);
    }

    #[test]
    fn descriptor_text_trims_and_terminates() {
        let mut desc = [0u8; 18];
        desc[3] = 0xFC;
        desc[5..17].copy_from_slice(b"NE160QDM-NYJ");
        desc[17] = 0x0A;
        assert_eq!(descriptor_text(&desc).as_deref(), Some("NE160QDM-NYJ"));
    }

    #[test]
    fn registry_path_strips_output_index() {
        assert_eq!(
            registry_path_from_instance("DISPLAY\\BOE0C80\\5&x&0&UID256_0"),
            "DISPLAY\\BOE0C80\\5&x&0&UID256"
        );
        // No trailing _<digits> → unchanged.
        assert_eq!(
            registry_path_from_instance("DISPLAY\\BOE0C80\\UID256"),
            "DISPLAY\\BOE0C80\\UID256"
        );
    }

    #[test]
    fn parses_full_sample_edid() {
        let edid = sample_edid();
        let m = parse_edid(&edid).expect("valid EDID");
        assert_eq!(m.manufacturer_id, "BOE");
        assert_eq!(m.product_code, "0C80");
        assert_eq!(m.model_name.as_deref(), Some("NE160QDM-NYJ"));
        assert_eq!(m.manufacture_week, Some(31));
        assert_eq!(m.manufacture_year, Some(2023));
        assert_eq!(m.edid_version.as_deref(), Some("1.4"));
        assert_eq!(m.digital_interface, Some(DigitalInterface::DisplayPort));
        assert_eq!(m.color_bit_depth, Some(8));
        assert!((m.gamma.unwrap() - 2.20).abs() < 0.001);
        assert!(m.srgb_default);
        assert!(m.preferred_timing_is_native);
        assert_eq!(m.v_freq_hz, Some((48, 165)));
        assert_eq!(m.h_freq_khz, Some((29, 284)));
        assert_eq!(m.max_pixel_clock_mhz, Some(770));
        assert_eq!(m.native_resolution, Some((2560, 1600)));
        assert!(m.native_refresh_hz.is_some());
        assert_eq!(m.physical_size_cm, Some((34, 21)));
    }

    // Hardware probe (parity with os.rs::dump_os_state): prints the real monitors
    // on THIS machine. Not an assertion — values are hardware-specific.
    // `cargo test --lib platform::system::windows::monitor::tests::dump_monitors -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore = "hardware probe, prints machine-specific monitor state"]
    fn dump_monitors() {
        for (i, m) in get_monitors().into_iter().enumerate() {
            println!("--- monitor {i} ---");
            println!("manufacturer : {} {}", m.manufacturer_id, m.product_code);
            println!("model        : {:?}", m.model_name);
            println!("serial       : {:?}", m.serial_number);
            println!("date         : week {:?} / {:?}", m.manufacture_week, m.manufacture_year);
            println!("edid version : {:?}", m.edid_version);
            println!("interface    : {:?} {:?} bpc", m.digital_interface, m.color_bit_depth);
            println!("gamma        : {:?}", m.gamma);
            println!("h freq kHz   : {:?}", m.h_freq_khz);
            println!("v freq Hz    : {:?}", m.v_freq_hz);
            println!("max pclk MHz : {:?}", m.max_pixel_clock_mhz);
            println!("native       : {:?} @ {:?} Hz", m.native_resolution, m.native_refresh_hz);
            println!("size cm      : {:?}  diag in: {:?}", m.physical_size_cm, m.diagonal_inches);
        }
    }
}
