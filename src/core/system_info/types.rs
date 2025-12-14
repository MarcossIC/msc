use serde::{Deserialize, Serialize};

/// Complete system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub gpu: Vec<GpuInfo>,
    pub motherboard: Option<MotherboardInfo>,
    pub network: NetworkInfo,
    pub storage: Vec<StorageInfo>,
    pub os: OsInfo,
    pub npu: Option<NpuInfo>,
    pub battery: Option<BatteryInfo>,      // Only on laptops
    pub power_plan: Option<PowerPlanInfo>, // Windows power settings
}

/// CPU Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub model: String,
    pub vendor: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub architecture: String,
    pub frequency_mhz: u64,
    pub max_frequency_mhz: Option<u64>,
    pub turbo_boost_enabled: Option<bool>, // Intel Turbo Boost / AMD Precision Boost

    // Cache information
    pub l1_cache_kb: Option<u32>,
    pub l2_cache_kb: Option<u32>,
    pub l3_cache_kb: Option<u32>,

    // Power and frequency
    pub tdp_watts: Option<u32>,
    pub max_tdp_watts: Option<u32>,
    pub turbo_frequency_mhz: Option<u64>,

    // Instruction set support
    pub instruction_sets: CpuInstructionSets,

    // Usage and performance
    pub current_usage_percent: Option<f32>,

    // Topology (AMD-specific)
    pub numa_nodes: Option<u32>,
    pub chiplet_count: Option<u32>,
    pub ccd_count: Option<u32>, // Core Complex Dies (Zen)
    pub ccx_count: Option<u32>, // Core Complexes (Zen)
}

/// CPU Instruction Set Support
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CpuInstructionSets {
    pub avx2: bool,
    pub avx512: bool,
    pub fma3: bool,
    pub sse4_2: bool,
    pub sha: bool,
    pub aes_ni: bool,
}

/// Memory Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
    pub ddr_type: Option<DdrType>,
    pub speed_mhz: Option<u32>,
    pub modules: Vec<MemoryModule>,
    pub total_slots: Option<u32>, // Total DIMM slots on motherboard
    pub used_slots: Option<u32>,  // Number of slots occupied
    pub max_capacity_bytes: Option<u64>, // Maximum RAM supported by motherboard (SMBIOS raw value)
    pub prediction: Option<MemoryPrediction>, // Advanced prediction analysis
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryModule {
    pub capacity_bytes: u64,
    pub speed_mhz: Option<u32>,
    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
    pub slot: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DdrType {
    DDR,
    DDR2,
    DDR3,
    DDR4,
    DDR5,
    Unknown,
}

impl std::fmt::Display for DdrType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DdrType::DDR => write!(f, "DDR"),
            DdrType::DDR2 => write!(f, "DDR2"),
            DdrType::DDR3 => write!(f, "DDR3"),
            DdrType::DDR4 => write!(f, "DDR4"),
            DdrType::DDR5 => write!(f, "DDR5"),
            DdrType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// GPU Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vram_bytes: Option<u64>,
    pub memory_type: Option<String>,
    pub is_integrated: bool,
    pub driver_version: Option<String>,

    // Common metrics
    pub core_clock_mhz: Option<u32>,
    pub memory_clock_mhz: Option<u32>,
    pub temperature_celsius: Option<u32>,
    pub power_draw_watts: Option<f32>,
    pub fan_speed_percent: Option<u32>,

    // NVIDIA-specific
    pub nvidia_metrics: Option<NvidiaGpuMetrics>,

    // AMD-specific
    pub amd_metrics: Option<AmdGpuMetrics>,
}

/// NVIDIA-specific GPU metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaGpuMetrics {
    // Static information (doesn't change)
    pub name: Option<String>,
    pub uuid: Option<String>,
    pub pci_bus_id: Option<String>,
    pub cuda_compute_capability: Option<(u32, u32)>, // (major, minor)
    pub memory_total_bytes: Option<u64>,
    pub memory_bus_width_bits: Option<u32>,
    pub cuda_cores: Option<u32>,
    pub sm_count: Option<u32>,            // Streaming Multiprocessors
    pub max_pcie_generation: Option<u32>, // PCIe Gen (3, 4, 5)
    pub max_pcie_lanes: Option<u32>,      // x8, x16

    // Static specs (estimated or from database)
    pub tdp_watts: Option<u32>,
    pub process_nm: Option<u32>, // Fabrication process (e.g., 4nm, 5nm)
    pub tensor_cores: Option<u32>,
    pub rt_cores: Option<u32>, // Ray Tracing cores

    // Dynamic information (real-time)
    pub temperature_celsius: Option<u32>,
    pub power_draw_watts: Option<f32>,
    pub power_limit_watts: Option<u32>,
    pub fan_speed_percent: Option<u32>,
    pub fan_rpm: Option<u32>,
    pub clock_graphics_mhz: Option<u32>,
    pub clock_memory_mhz: Option<u32>,
    pub clock_sm_mhz: Option<u32>,
    pub utilization_gpu_percent: Option<u32>,
    pub utilization_memory_percent: Option<u32>,
    pub memory_used_bytes: Option<u64>,
    pub memory_free_bytes: Option<u64>,
    pub current_pcie_generation: Option<u32>, // Currently negotiated PCIe gen
    pub current_pcie_lanes: Option<u32>,      // Currently negotiated lanes
    pub pstate: Option<String>,               // Performance state (P0-P15)
    pub throttle_reasons: Option<Vec<String>>,
}

/// AMD-specific GPU metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdGpuMetrics {
    pub shader_count: Option<u32>,
    pub compute_units: Option<u32>,
    pub rop_count: Option<u32>,              // Render Output Units
    pub tmu_count: Option<u32>,              // Texture Mapping Units
    pub smartshift_power_watts: Option<f32>, // SmartShift allocated power (laptops)
    pub infinity_cache_mb: Option<u32>,      // Infinity Cache size
    pub process_nm: Option<u32>,             // Fabrication process
}

/// Motherboard Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotherboardInfo {
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub version: Option<String>, // Board revision (e.g., "Rev 1.0")
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub chipset: Option<String>, // Chipset name (e.g., "Intel Z790", "AMD X670")
    pub tpm_version: Option<TpmVersion>, // TPM version (1.2, 2.0)
    pub dimm_slots: Option<u32>, // Total DIMM/RAM slots
    pub pcie_slots: Option<Vec<PcieSlot>>, // PCIe slot information
    pub m2_slots_total: Option<u32>, // Total M.2 slots
    pub m2_slots_used: Option<u32>, // M.2 slots in use
}

/// TPM (Trusted Platform Module) Version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TpmVersion {
    V1_2,
    V2_0,
    Unknown,
}

impl std::fmt::Display for TpmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TpmVersion::V1_2 => write!(f, "TPM 1.2"),
            TpmVersion::V2_0 => write!(f, "TPM 2.0"),
            TpmVersion::Unknown => write!(f, "Unknown"),
        }
    }
}

/// PCIe Slot Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcieSlot {
    pub slot_type: PcieSlotType, // x16, x8, x4, x1
    pub version: Option<String>, // PCIe 3.0, 4.0, 5.0
    pub occupied: bool,          // Is slot in use
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PcieSlotType {
    X16,
    X8,
    X4,
    X1,
    Unknown,
}

impl std::fmt::Display for PcieSlotType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PcieSlotType::X16 => write!(f, "x16"),
            PcieSlotType::X8 => write!(f, "x8"),
            PcieSlotType::X4 => write!(f, "x4"),
            PcieSlotType::X1 => write!(f, "x1"),
            PcieSlotType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Network Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub wifi_adapters: Vec<WifiAdapter>,
    pub ethernet_adapters: Vec<EthernetAdapter>,
    pub bluetooth_adapters: Vec<BluetoothAdapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiAdapter {
    pub name: String,
    pub wifi_standard: WifiStandard,
    pub bands: Vec<WifiBand>,
    pub max_speed_mbps: Option<u32>,
    pub current_link_speed_mbps: Option<u32>, // Current connection speed
    pub chipset_manufacturer: Option<String>, // Realtek, Intel, Qualcomm, etc.
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub gateway_latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WifiStandard {
    WiFi4,  // 802.11n
    WiFi5,  // 802.11ac
    WiFi6,  // 802.11ax (2.4/5 GHz)
    WiFi6E, // 802.11ax (6 GHz)
    WiFi7,  // 802.11be
    Unknown,
}

impl std::fmt::Display for WifiStandard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WifiStandard::WiFi4 => write!(f, "WiFi 4 (802.11n)"),
            WifiStandard::WiFi5 => write!(f, "WiFi 5 (802.11ac)"),
            WifiStandard::WiFi6 => write!(f, "WiFi 6 (802.11ax)"),
            WifiStandard::WiFi6E => write!(f, "WiFi 6E (802.11ax)"),
            WifiStandard::WiFi7 => write!(f, "WiFi 7 (802.11be)"),
            WifiStandard::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WifiBand {
    Band2_4GHz,
    Band5GHz,
    Band6GHz,
}

impl std::fmt::Display for WifiBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WifiBand::Band2_4GHz => write!(f, "2.4 GHz"),
            WifiBand::Band5GHz => write!(f, "5 GHz"),
            WifiBand::Band6GHz => write!(f, "6 GHz"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthernetAdapter {
    pub name: String,
    pub speed_mbps: Option<u32>,
    pub mac_address: Option<String>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub gateway_latency_ms: Option<u32>,
}

/// Bluetooth Adapter Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothAdapter {
    pub name: String,
    pub version: BluetoothVersion,
    pub is_enabled: bool,
    pub manufacturer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BluetoothVersion {
    V4_0,
    V4_1,
    V4_2,
    V5_0,
    V5_1,
    V5_2,
    V5_3,
    V5_4,
    Unknown,
}

impl std::fmt::Display for BluetoothVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BluetoothVersion::V4_0 => write!(f, "Bluetooth 4.0"),
            BluetoothVersion::V4_1 => write!(f, "Bluetooth 4.1"),
            BluetoothVersion::V4_2 => write!(f, "Bluetooth 4.2"),
            BluetoothVersion::V5_0 => write!(f, "Bluetooth 5.0"),
            BluetoothVersion::V5_1 => write!(f, "Bluetooth 5.1"),
            BluetoothVersion::V5_2 => write!(f, "Bluetooth 5.2"),
            BluetoothVersion::V5_3 => write!(f, "Bluetooth 5.3"),
            BluetoothVersion::V5_4 => write!(f, "Bluetooth 5.4"),
            BluetoothVersion::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Storage Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
    pub disk_type: DiskType,
    pub file_system: String,

    // Physical disk information
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub firmware_version: Option<String>,

    // Bus and interface
    pub bus_type: Option<BusType>,
    pub interface_speed: Option<InterfaceSpeed>,

    // SMART data
    pub smart_status: Option<SmartStatus>,
    pub temperature_celsius: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub total_bytes_read: Option<u64>,
    pub total_bytes_written: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiskType {
    HDD,
    SSD,
    NVMe,
    Unknown,
}

impl std::fmt::Display for DiskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiskType::HDD => write!(f, "HDD"),
            DiskType::SSD => write!(f, "SSD"),
            DiskType::NVMe => write!(f, "NVMe SSD"),
            DiskType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BusType {
    SATA,
    NVMe,
    USB,
    SCSI,
    Unknown,
}

impl std::fmt::Display for BusType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BusType::SATA => write!(f, "SATA"),
            BusType::NVMe => write!(f, "NVMe"),
            BusType::USB => write!(f, "USB"),
            BusType::SCSI => write!(f, "SCSI"),
            BusType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InterfaceSpeed {
    // SATA
    SATA1_5Gbps,
    SATA3Gbps,
    SATA6Gbps,

    // PCIe NVMe
    PCIe3x2, // PCIe 3.0 x2 lanes
    PCIe3x4, // PCIe 3.0 x4 lanes
    PCIe4x4, // PCIe 4.0 x4 lanes
    PCIe5x4, // PCIe 5.0 x4 lanes

    // USB
    USB3_5Gbps,
    USB3_10Gbps,
    USB3_20Gbps,

    Unknown,
}

impl std::fmt::Display for InterfaceSpeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterfaceSpeed::SATA1_5Gbps => write!(f, "SATA 1.5 Gb/s"),
            InterfaceSpeed::SATA3Gbps => write!(f, "SATA 3 Gb/s"),
            InterfaceSpeed::SATA6Gbps => write!(f, "SATA 6 Gb/s (SATA III)"),
            InterfaceSpeed::PCIe3x2 => write!(f, "PCIe 3.0 x2 (~2 GB/s)"),
            InterfaceSpeed::PCIe3x4 => write!(f, "PCIe 3.0 x4 (~4 GB/s)"),
            InterfaceSpeed::PCIe4x4 => write!(f, "PCIe 4.0 x4 (~8 GB/s)"),
            InterfaceSpeed::PCIe5x4 => write!(f, "PCIe 5.0 x4 (~16 GB/s)"),
            InterfaceSpeed::USB3_5Gbps => write!(f, "USB 3.0 (5 Gb/s)"),
            InterfaceSpeed::USB3_10Gbps => write!(f, "USB 3.1 Gen 2 (10 Gb/s)"),
            InterfaceSpeed::USB3_20Gbps => write!(f, "USB 3.2 Gen 2x2 (20 Gb/s)"),
            InterfaceSpeed::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SmartStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

impl std::fmt::Display for SmartStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmartStatus::Healthy => write!(f, "Healthy"),
            SmartStatus::Warning => write!(f, "Warning"),
            SmartStatus::Critical => write!(f, "Critical"),
            SmartStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Operating System Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub build: Option<String>,
    pub architecture: String,
    pub kernel_version: Option<String>,
}

/// NPU Information (Neural Processing Unit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpuInfo {
    pub name: String,
    pub tops: Option<f32>, // Tera Operations Per Second
}

/// Battery Information (Laptops only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub is_present: bool,
    pub state: BatteryState,
    pub percentage: Option<u8>,                // 0-100%
    pub time_remaining_secs: Option<u32>,      // Seconds remaining (if discharging)
    pub time_to_full_secs: Option<u32>,        // Seconds to full charge (if charging)
    pub design_capacity_mwh: Option<u32>,      // Original capacity in mWh
    pub full_charge_capacity_mwh: Option<u32>, // Current full capacity
    pub health_percentage: Option<u8>,         // (full_charge / design) * 100
    pub cycle_count: Option<u32>,              // Number of charge cycles
    pub technology: Option<BatteryTechnology>,
    pub voltage_mv: Option<u32>,        // Current voltage in mV
    pub design_voltage_mv: Option<u32>, // Design voltage in mV
    pub discharge_rate_mw: Option<i32>, // Current discharge rate (negative if charging)
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
    pub manufacture_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    NotCharging, // Plugged in but not charging (already at 100%)
    Unknown,
}

impl std::fmt::Display for BatteryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatteryState::Charging => write!(f, "Charging"),
            BatteryState::Discharging => write!(f, "Discharging"),
            BatteryState::Full => write!(f, "Full"),
            BatteryState::NotCharging => write!(f, "Not Charging"),
            BatteryState::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BatteryTechnology {
    LithiumIon,         // Li-ion
    LithiumPolymer,     // Li-Po
    NickelMetalHydride, // NiMH
    NickelCadmium,      // NiCd
    LeadAcid,
    Unknown,
}

impl std::fmt::Display for BatteryTechnology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatteryTechnology::LithiumIon => write!(f, "Lithium-Ion (Li-ion)"),
            BatteryTechnology::LithiumPolymer => write!(f, "Lithium-Polymer (Li-Po)"),
            BatteryTechnology::NickelMetalHydride => write!(f, "Nickel-Metal Hydride (NiMH)"),
            BatteryTechnology::NickelCadmium => write!(f, "Nickel-Cadmium (NiCd)"),
            BatteryTechnology::LeadAcid => write!(f, "Lead-Acid"),
            BatteryTechnology::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Power Plan Information (Windows)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerPlanInfo {
    pub active_plan: PowerPlan,
    pub power_mode: Option<PowerMode>,   // Windows 10+ power slider
    pub processor_min_state: Option<u8>, // % minimum CPU state
    pub processor_max_state: Option<u8>, // % maximum CPU state
    pub display_brightness_ac: Option<u8>, // % brightness on AC
    pub display_brightness_dc: Option<u8>, // % brightness on battery
    pub display_timeout_ac_secs: Option<u32>, // Seconds to turn off display on AC
    pub display_timeout_dc_secs: Option<u32>, // Seconds to turn off display on battery
    pub sleep_timeout_ac_secs: Option<u32>, // Seconds to sleep on AC
    pub sleep_timeout_dc_secs: Option<u32>, // Seconds to sleep on battery
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PowerPlan {
    Balanced,
    HighPerformance,
    PowerSaver,
    UltimatePerformance, // Windows 10+
    Custom(String),      // Custom power plan
}

impl std::fmt::Display for PowerPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerPlan::Balanced => write!(f, "Balanced"),
            PowerPlan::HighPerformance => write!(f, "High Performance"),
            PowerPlan::PowerSaver => write!(f, "Power Saver"),
            PowerPlan::UltimatePerformance => write!(f, "Ultimate Performance"),
            PowerPlan::Custom(name) => write!(f, "Custom: {}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PowerMode {
    BestPowerEfficiency, // Best battery life
    BetterBattery,
    BetterPerformance,
    BestPerformance,
}

impl std::fmt::Display for PowerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerMode::BestPowerEfficiency => write!(f, "Best Power Efficiency"),
            PowerMode::BetterBattery => write!(f, "Better Battery"),
            PowerMode::BetterPerformance => write!(f, "Better Performance"),
            PowerMode::BestPerformance => write!(f, "Best Performance"),
        }
    }
}

/// Memory Capacity Prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPrediction {
    // Capacities
    pub minimum_guaranteed_bytes: u64, // Based on installed modules
    pub maximum_predicted_bytes: u64,  // Final prediction
    pub smbios_reported_bytes: Option<u64>, // SMBIOS value

    // Limitations
    pub cpu_limit_bytes: Option<u64>,     // CPU IMC limit
    pub ddr_generation_limit_bytes: u64,  // DDR generation limit
    pub chipset_limit_bytes: Option<u64>, // Chipset limit

    // Confidence and justification
    pub confidence: ConfidenceLevel,
    pub justification: Vec<String>, // Prediction reasons
    pub warnings: Vec<String>,      // User warnings

    // Additional data
    pub is_smbios_validated: bool, // Is SMBIOS value valid?
    pub prediction_method: PredictionMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceLevel {
    High,   // 90-100%: Known CPU + Known DDR + Valid SMBIOS
    Medium, // 70-89%: Known CPU + Known DDR
    Low,    // <70%: Fallback to general heuristics
}

impl std::fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfidenceLevel::High => write!(f, "High"),
            ConfidenceLevel::Medium => write!(f, "Medium"),
            ConfidenceLevel::Low => write!(f, "Low"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PredictionMethod {
    CpuAndDdrLimits, // Based on known CPU and DDR limits
    DdrAndSlots,     // Based on DDR type and slot count
    SmbiosValidated, // Validated and reliable SMBIOS
    Conservative,    // Conservative estimation
}

impl std::fmt::Display for PredictionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PredictionMethod::CpuAndDdrLimits => write!(f, "CPU and DDR Limits"),
            PredictionMethod::DdrAndSlots => write!(f, "DDR Generation and Slots"),
            PredictionMethod::SmbiosValidated => write!(f, "Validated SMBIOS"),
            PredictionMethod::Conservative => write!(f, "Conservative Estimation"),
        }
    }
}

/// CPU Memory Controller Limits
#[derive(Debug, Clone)]
pub struct CpuMemoryLimits {
    pub max_capacity_gb: u32,
    pub max_channels: u8,
    pub supported_ddr_types: Vec<DdrType>,
}

/// DDR Generation Limits (JEDEC Standards)
#[derive(Debug, Clone)]
pub struct DdrGenerationLimits {
    pub ddr_type: DdrType,
    pub max_module_capacity_gb: u32,    // Maximum capacity per DIMM
    pub common_capacities_gb: Vec<u32>, // Common available capacities
}
