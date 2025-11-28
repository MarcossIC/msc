// Integration tests module

mod integration {
    mod clean_commands_test;
    mod clean_paths_test;
    mod commands_test;
    mod config_test;
    mod ffmpeg_test;
    mod set_command_test;
    mod vget_security_test;
    mod wget_test;
    mod workspace_test;

    // Security audit tests
    mod alias_security_test;
    mod fuzzing_security_test;
    mod security_audit_test;
}
