// Test rápido para verificar detección de GPU
#[cfg(feature = "nvml")]
use nvml_wrapper::Nvml;

fn main() {
    println!("=== GPU Detection Test ===\n");

    #[cfg(feature = "nvml")]
    {
        println!("✓ NVML feature is enabled");

        match Nvml::init() {
            Ok(nvml) => {
                println!("✓ NVML initialized successfully");

                match nvml.device_count() {
                    Ok(count) => {
                        println!("✓ Found {} GPU(s)", count);

                        for i in 0..count {
                            match nvml.device_by_index(i) {
                                Ok(device) => {
                                    let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                                    println!("  GPU {}: {}", i, name);
                                }
                                Err(e) => {
                                    println!("  ✗ Failed to get GPU {}: {:?}", i, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("✗ Failed to get device count: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("✗ NVML initialization failed: {:?}", e);
                println!("\nPossible causes:");
                println!("  - NVIDIA drivers not installed");
                println!("  - NVML library (nvml.dll) not found");
                println!("  - Incompatible driver version");
                println!("  - No NVIDIA GPU present");
            }
        }
    }

    #[cfg(not(feature = "nvml"))]
    {
        println!("✗ NVML feature is NOT enabled");
        println!("  Compile with: cargo build --features nvml");
    }

    println!("\n=== End of Test ===");
}
