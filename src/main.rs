use anyhow::Result;
use clap::{Arg, Command};
use msc::commands;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting MSC CLI v{}", env!("CARGO_PKG_VERSION"));

    let matches = build_cli().get_matches();

    // Handle version flag
    if matches.get_flag("version") {
        commands::version::execute()?;
        return Ok(());
    }

    // Dispatch commands
    match matches.subcommand() {
        Some(("hello", sub_matches)) => commands::hello::execute(sub_matches),
        Some(("version", _)) => commands::version::execute(),
        Some(("set", sub_matches)) => commands::config::handle_set(sub_matches),
        Some(("get", sub_matches)) => commands::config::handle_get(sub_matches),
        Some(("work", sub_matches)) => commands::workspace::execute(sub_matches),
        Some(("alias", sub_matches)) => commands::alias::handle_alias(sub_matches),
        Some(("clean", sub_matches)) => match sub_matches.subcommand() {
            Some(("start", sub_sub_matches)) => commands::clean::handle_start(sub_sub_matches),
            Some(("add", sub_sub_matches)) => commands::clean::handle_add(sub_sub_matches),
            Some(("list", sub_sub_matches)) => commands::clean::handle_list(sub_sub_matches),
            Some(("remove", sub_sub_matches)) => commands::clean::handle_remove(sub_sub_matches),
            Some(("reset", sub_sub_matches)) => commands::clean::handle_clear(sub_sub_matches),
            Some(("ignore", sub_sub_matches)) => match sub_sub_matches.subcommand() {
                Some(("add", ignore_matches)) => commands::clean::handle_ignore_add(ignore_matches),
                Some(("list", ignore_matches)) => {
                    commands::clean::handle_ignore_list(ignore_matches)
                }
                Some(("remove", ignore_matches)) => {
                    commands::clean::handle_ignore_remove(ignore_matches)
                }
                _ => {
                    println!("Use 'msc clean ignore --help' for more information.");
                    Ok(())
                }
            },
            _ => {
                println!("Use 'msc clean --help' for more information.");
                Ok(())
            }
        },
        Some(("list", sub_matches)) => commands::list::execute(sub_matches),
        _ => {
            println!("Welcome to MSC CLI!");
            println!("Use 'msc --help' for more information.");
            Ok(())
        }
    }
}

fn build_cli() -> Command {
    Command::new("msc")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Marco")
        .about("A modular command-line interface tool")
        .disable_version_flag(true)
        .arg(
            Arg::new("version")
                .short('v')
                .short_alias('V')
                .long("version")
                .help("Print version information")
                .action(clap::ArgAction::SetTrue),
        )
        .subcommand(
            Command::new("hello").about("Says hello").arg(
                Arg::new("name")
                    .short('n')
                    .long("name")
                    .value_name("NAME")
                    .help("Name to greet")
                    .default_value("World"),
            ),
        )
        .subcommand(Command::new("version").about("Shows version information"))
        .subcommand(
            Command::new("list")
                .about("List files and directories")
                .arg(
                    Arg::new("path")
                        .help("Directory to list (defaults to current directory)")
                        .index(1),
                )
                .arg(
                    Arg::new("all")
                        .short('a')
                        .long("all")
                        .help("Show hidden files")
                        .action(clap::ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("deep")
                        .short('d')
                        .long("deep")
                        .help("List files recursively (default depth: 1)")
                        .action(clap::ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("depth")
                        .long("depth")
                        .help("Maximum depth to traverse when using --deep")
                        .value_parser(clap::value_parser!(u32))
                        .default_value("1")
                        .requires("deep"),
                )
                .arg(
                    Arg::new("long")
                        .short('l')
                        .long("long")
                        .help("Use long listing format (table view)")
                        .action(clap::ArgAction::SetTrue),
                )
                .subcommand(
                    Command::new("deep")
                        .about("List files and directories recursively")
                        .arg(
                            Arg::new("path")
                                .help("Directory to list (defaults to current directory)")
                                .index(1),
                        )
                        .arg(
                            Arg::new("all")
                                .short('a')
                                .long("all")
                                .help("Show hidden files")
                                .action(clap::ArgAction::SetTrue),
                        )
                        .arg(
                            Arg::new("depth")
                                .short('d')
                                .long("depth")
                                .help("Maximum depth to traverse (default: 1)")
                                .value_parser(clap::value_parser!(u32))
                                .default_value("1"),
                        ),
                ),
        )
        .subcommand(
            Command::new("set")
                .about("Set configuration values")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("work").about("Set work directory path").arg(
                        Arg::new("path")
                            .help("Path to the work directory")
                            .required(true)
                            .index(1),
                    ),
                ),
        )
        .subcommand(
            Command::new("get")
                .about("Get configuration values")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(Command::new("work").about("Get work directory path")),
        )
        .subcommand(
            Command::new("work")
                .about("Manage workspaces")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(Command::new("map").about("Map project folders as workspaces"))
                .subcommand(Command::new("list").about("List all registered workspaces")),
        )
        .subcommand(
            Command::new("alias")
                .about("Manage global command aliases")
                .long_about(
                    "Create and manage global command aliases.\n\n\
                    SUBCOMMANDS:\n\
                    add     - Create a new alias\n\
                    remove  - Remove an existing alias\n\
                    list    - List all configured aliases\n\
                    init    - Initialize alias system (add to PATH)\n\
                    nuke    - Completely remove alias system and configuration\n\n\
                    EXAMPLES:\n\
                    msc alias add pyh \"python3 -m http.server 5000\"  # Create alias\n\
                    msc alias list                                      # List all aliases\n\
                    msc alias remove pyh                                # Remove alias\n\
                    msc alias init                                      # Setup PATH\n\
                    msc alias nuke                                      # Clean everything"
                )
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("add")
                        .about("Create a new alias")
                        .arg(
                            Arg::new("name")
                                .help("Name of the alias")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("command")
                                .help("Command to execute")
                                .required(true)
                                .index(2),
                        )
                        .arg(
                            Arg::new("description")
                                .short('d')
                                .long("description")
                                .help("Optional description for the alias")
                                .value_name("DESC"),
                        ),
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove an existing alias")
                        .arg(
                            Arg::new("name")
                                .help("Name of the alias to remove")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List all configured aliases")
                )
                .subcommand(
                    Command::new("init")
                        .about("Initialize alias system and add to PATH")
                )
                .subcommand(
                    Command::new("nuke")
                        .about("Completely remove alias system and configuration")
                        .long_about(
                            "⚠️  WARNING: This will completely remove all alias configuration!\n\n\
                            This command will:\n\
                            • Remove all alias executables\n\
                            • Delete the alias configuration file\n\
                            • Remove the aliases directory from your PATH\n\
                            • Delete the entire aliases directory\n\n\
                            Use this for a clean reset if you want to start fresh or uninstall the alias system.\n\n\
                            EXAMPLES:\n\
                            msc alias nuke    # Clean everything (asks for confirmation)"
                        )
                ),
        )
        .subcommand(
            Command::new("clean")
                .about("Manage cleanup operations and clean paths")
                .long_about(
                    "Manage cleanup operations and clean paths.\n\n\
                    SUBCOMMANDS:\n\
                    start   - Clean temporary files from configured paths\n\
                    list    - List all active clean paths (default + custom)\n\
                    add     - Add a custom directory to clean paths\n\
                    remove  - Remove a custom clean path (interactive)\n\
                    reset   - Reset to default clean paths only\n\
                    ignore  - Manage ignored folders for work cache cleanup\n\n\
                    QUICK START:\n\
                    msc clean list                  # See what directories will be cleaned\n\
                    msc clean start --dry-run       # Preview what would be deleted\n\
                    msc clean start                 # Clean temporary files (safe mode)\n\
                    msc clean add <path>            # Add custom directory to clean\n\n\
                    Use 'msc clean <SUBCOMMAND> --help' for more information on each subcommand."
                )
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("start")
                        .about("Clean temporary files from the system")
                        .long_about(
                            "Clean temporary files from configured paths.\n\n\
                            SAFETY FEATURES:\n\
                            • Two-phase cleanup (user dirs first, then system dirs with admin)\n\
                            • Only files older than 24 hours are deleted by default\n\
                            • Ctrl+C anytime to cancel safely\n\
                            • Dry-run mode to preview changes\n\n\
                            FLAGS:\n\
                            --dry-run              Simulate cleanup without deleting files\n\
                            --min-age <HOURS>      Only delete files older than N hours (default: 24)\n\
                            --include-recent       Delete files of all ages (⚠️  DANGEROUS!)\n\
                            --include-recycle      Include Recycle Bin in cleanup\n\
                            --IR                   Alias for --include-recycle\n\
                            --work-cache, -WC      Clean cache folders in work directory projects\n\n\
                            EXAMPLES:\n\
                            msc clean start                      # Clean files older than 24 hours\n\
                            msc clean start --dry-run            # Preview what would be deleted\n\
                            msc clean start --min-age 48         # Only delete files older than 48 hours\n\
                            msc clean start --include-recent     # Delete all files (⚠️  dangerous!)\n\
                            msc clean start --IR                 # Include Recycle Bin in cleanup\n\
                            msc clean start --include-recycle    # Same as --IR\n\
                            msc clean start --work-cache         # Clean cache folders in work projects\n\
                            msc clean start -WC                  # Same as --work-cache"
                        )
                        .arg(
                            Arg::new("dry-run")
                                .long("dry-run")
                                .help("Show what would be deleted without actually deleting")
                                .action(clap::ArgAction::SetTrue),
                        )
                        .arg(
                            Arg::new("min-age")
                                .long("min-age")
                                .value_name("HOURS")
                                .help("Only delete files older than N hours (default: 24)")
                                .value_parser(clap::value_parser!(u64)),
                        )
                        .arg(
                            Arg::new("include-recent")
                                .long("include-recent")
                                .help("Include recently modified files (⚠️  dangerous!)")
                                .action(clap::ArgAction::SetTrue)
                                .conflicts_with("min-age"),
                        )
                        .arg(
                            Arg::new("include-recycle")
                                .long("include-recycle")
                                .visible_alias("IR")
                                .help("Include Recycle Bin in cleanup (alias: --IR)")
                                .action(clap::ArgAction::SetTrue),
                        )
                        .arg(
                            Arg::new("work-cache")
                                .long("work-cache")
                                .visible_alias("WC")
                                .help("Clean cache folders (target, dist, node_modules) in work directory projects")
                                .action(clap::ArgAction::SetTrue),
                        ),
                )
                .subcommand(
                    Command::new("add")
                        .about("Add a custom directory to clean paths")
                        .long_about(
                            "Add a custom directory to be included in cleanup operations.\n\n\
                            The path will be validated for safety before being added.\n\
                            Protected system directories cannot be added.\n\n\
                            FLAGS:\n\
                            -f, --force    Skip safety warnings (⚠️  dangerous!)\n\n\
                            EXAMPLES:\n\
                            msc clean add C:\\MyTempFolder           # Add custom temp directory\n\
                            msc clean add D:\\Downloads\\Temp        # Add another custom path\n\
                            msc clean add C:\\Temp --force          # Force add (skip warnings)"
                        )
                        .arg(
                            Arg::new("path")
                                .help("Directory path to add to clean paths")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("force")
                                .short('f')
                                .long("force")
                                .help("Skip safety warnings (⚠️  dangerous!)")
                                .action(clap::ArgAction::SetTrue),
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List all active clean paths")
                        .long_about(
                            "Display all directories that will be cleaned during cleanup operations.\n\n\
                            This includes:\n\
                            • Default system temporary directories\n\
                            • Custom directories you've added\n\n\
                            EXAMPLES:\n\
                            msc clean list    # Show all configured clean paths"
                        )
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove a custom clean path")
                        .long_about(
                            "Remove a directory from clean paths using interactive selection.\n\n\
                            You can only remove custom paths, not default system paths.\n\
                            Use 'msc clean reset' to restore default configuration.\n\n\
                            EXAMPLES:\n\
                            msc clean remove    # Interactive selection to remove a path"
                        )
                )
                .subcommand(
                    Command::new("reset")
                        .about("Reset to default clean paths only")
                        .long_about(
                            "Reset clean paths configuration to system defaults.\n\n\
                            This will:\n\
                            • Remove all custom paths you've added\n\
                            • Restore default system temporary directories\n\n\
                            EXAMPLES:\n\
                            msc clean reset    # Reset to default configuration"
                        )
                )
                .subcommand(
                    Command::new("ignore")
                        .about("Manage ignored folders for work cache cleanup")
                        .long_about(
                            "Manage folders that should be ignored during work cache cleanup.\n\n\
                            When using --work-cache flag, these folders will be skipped.\n\
                            Note: 'msc' folder is always ignored automatically.\n\n\
                            SUBCOMMANDS:\n\
                            add     - Add a folder to ignore list\n\
                            list    - List all ignored folders\n\
                            remove  - Remove a folder from ignore list\n\n\
                            EXAMPLES:\n\
                            msc clean ignore list              # Show ignored folders\n\
                            msc clean ignore add my-project    # Ignore 'my-project' folder\n\
                            msc clean ignore remove my-project # Stop ignoring 'my-project'"
                        )
                        .subcommand_required(true)
                        .arg_required_else_help(true)
                        .subcommand(
                            Command::new("add")
                                .about("Add a folder to the ignore list")
                                .arg(
                                    Arg::new("folder")
                                        .help("Folder name to ignore")
                                        .required(true)
                                        .index(1),
                                ),
                        )
                        .subcommand(
                            Command::new("list")
                                .about("List all ignored folders")
                        )
                        .subcommand(
                            Command::new("remove")
                                .about("Remove a folder from the ignore list")
                                .arg(
                                    Arg::new("folder")
                                        .help("Folder name to stop ignoring")
                                        .required(true)
                                        .index(1),
                                ),
                        ),
                ),
        )
}
