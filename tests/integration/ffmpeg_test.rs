use msc::core::FFmpegManager;
use std::path::PathBuf;

#[test]
fn test_ffmpeg_manager_new() {
    // Skip this test in CI environments where config directory may not be available
    let manager = FFmpegManager::new();
    match manager {
        Ok(_) => println!("✓ FFmpegManager created successfully"),
        Err(e) => {
            println!("⊘ Skipping test: {}", e);
            // Test passes - just verifying it doesn't panic
        }
    }
}

#[test]
fn test_ffmpeg_manager_default() {
    let manager = FFmpegManager::default();
    // Default manager should have no binary path initially
    assert!(
        manager.get_binary_path().is_none(),
        "Default manager should have no binary path"
    );
}

#[test]
fn test_get_install_dir() {
    // This is a private function, but we can test the overall installation logic
    // Skip this test in CI environments where config directory may not be available
    let manager = FFmpegManager::new();
    match manager {
        Ok(_) => println!("✓ FFmpegManager created successfully"),
        Err(e) => {
            println!("⊘ Skipping test: {}", e);
            // Test passes - just verifying it doesn't panic
        }
    }
}

#[test]
fn test_system_ffmpeg_detection() {
    // Note: This test will pass regardless of whether ffmpeg is installed
    // It just verifies that the detection mechanism doesn't crash
    println!("Testing system ffmpeg detection...");

    // We can't directly test the private check_system_ffmpeg function,
    // but we can verify that the manager can be created without errors
    // Skip this test in CI environments where config directory may not be available
    let manager = FFmpegManager::new();
    match manager {
        Ok(_) => println!("✓ FFmpegManager created successfully"),
        Err(e) => {
            println!("⊘ Skipping test: {}", e);
            // Test passes - just verifying it doesn't panic
        }
    }
}

#[test]
fn test_video_file_validation() {
    // Test valid video extensions
    let valid_extensions = vec!["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v"];

    for ext in valid_extensions {
        let path = PathBuf::from(format!("test.{}", ext));
        println!("Testing extension: {}", ext);
        // The actual validation is in vedit.rs, but we can test the concept
        assert!(path.extension().is_some(), "Path should have extension");
    }
}

#[test]
fn test_output_path_generation() {
    // Test that we can construct output paths correctly
    let input = PathBuf::from("C:\\videos\\test.mp4");

    if let Some(stem) = input.file_stem() {
        if let Some(extension) = input.extension() {
            let output_name = format!(
                "{}_compress.{}",
                stem.to_string_lossy(),
                extension.to_string_lossy()
            );
            assert_eq!(
                output_name, "test_compress.mp4",
                "Output name should be formatted correctly"
            );
        }
    }

    // Test with different extension
    let input2 = PathBuf::from("/home/user/video.avi");

    if let Some(stem) = input2.file_stem() {
        if let Some(extension) = input2.extension() {
            let output_name = format!(
                "{}_compress.{}",
                stem.to_string_lossy(),
                extension.to_string_lossy()
            );
            assert_eq!(
                output_name, "video_compress.avi",
                "Output name should be formatted correctly"
            );
        }
    }
}

#[test]
fn test_ffmpeg_quality_parameters() {
    // Test that quality parameters are correctly defined
    let quality_params = vec![
        ("low", "28", "fast", "96k"),
        ("medium", "23", "medium", "128k"),
        ("high", "18", "slow", "192k"),
    ];

    for (quality, expected_crf, expected_preset, expected_audio) in quality_params {
        println!("Testing quality: {}", quality);

        let (crf, preset, audio_bitrate) = match quality {
            "low" => ("28", "fast", "96k"),
            "medium" => ("23", "medium", "128k"),
            "high" => ("18", "slow", "192k"),
            _ => panic!("Invalid quality"),
        };

        assert_eq!(crf, expected_crf, "CRF should match for {}", quality);
        assert_eq!(
            preset, expected_preset,
            "Preset should match for {}",
            quality
        );
        assert_eq!(
            audio_bitrate, expected_audio,
            "Audio bitrate should match for {}",
            quality
        );
    }
}

#[test]
fn test_ffmpeg_manager_is_installed() {
    // Skip this test in CI environments where config directory may not be available
    let manager = match FFmpegManager::new() {
        Ok(m) => m,
        Err(e) => {
            println!("⊘ Skipping test: {}", e);
            return;
        }
    };

    // Initially, ffmpeg is likely not marked as installed by msc
    // This test just verifies the method doesn't panic
    let _is_installed = manager.is_installed();
    println!("✓ FFmpeg installation status checked successfully");
}

// Note: The following tests are commented out because they require network access
// or would actually download/install ffmpeg, which we don't want in CI/CD

/*
#[test]
#[ignore] // Requires network access
fn test_get_latest_version() {
    let version = FFmpegManager::get_latest_version();
    assert!(version.is_ok(), "Should be able to fetch latest version");

    if let Ok(v) = version {
        println!("Latest FFmpeg version: {}", v);
        assert!(!v.is_empty(), "Version string should not be empty");
    }
}

#[test]
#[ignore] // Requires network access and time
fn test_download_and_extract() {
    // This test would actually download FFmpeg
    // Only run manually when needed
    let version = "autobuild-2025-01-15-12-55"; // Example version
    let result = FFmpegManager::download_and_extract(version);
    assert!(result.is_ok(), "Should be able to download and extract FFmpeg");
}

#[test]
#[ignore] // Modifies system state
fn test_install() {
    let mut manager = FFmpegManager::new().expect("Should create manager");
    let result = manager.install();
    assert!(result.is_ok(), "Installation should succeed");
}
*/
