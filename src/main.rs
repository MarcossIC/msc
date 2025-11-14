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
        Some(("clean", sub_matches)) => match sub_matches.subcommand() {
            Some(("start", sub_sub_matches)) => commands::clean::handle_start(sub_sub_matches),
            Some(("add", sub_sub_matches)) => commands::clean::handle_add(sub_sub_matches),
            Some(("list", sub_sub_matches)) => commands::clean::handle_list(sub_sub_matches),
            Some(("remove", sub_sub_matches)) => commands::clean::handle_remove(sub_sub_matches),
            Some(("reset", sub_sub_matches)) => commands::clean::handle_clear(sub_sub_matches),
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
            Command::new("clean")
                .about("Manage cleanup operations and clean paths")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("start")
                        .about("Clean temporary files from the system")
                        .long_about(
                            "Clean temporary files from the system.\n\n\
                            Uses two-phase cleanup following the Principle of Least Privilege:\n\
                            1. User directories are cleaned WITHOUT admin privileges\n\
                            2. System directories are cleaned WITH admin privileges (after confirmation)\n\n\
                            By default, only files older than 24 hours are deleted to protect active applications.\n\n\
                            Options:\n\
                            --dry-run           Simulate cleanup without deleting files\n\
                            --min-age <HOURS>   Only delete files older than N hours (default: 24)\n\
                            --include-recent    Delete files of all ages (dangerous!)\n\
                            --include-recycle   Include Recycle Bin in cleanup\n\n\
                            Examples:\n\
                            msc clean start                      # Clean files older than 24 hours\n\
                            msc clean start --dry-run            # Preview what would be deleted\n\
                            msc clean start --min-age 48         # Only delete files older than 48 hours\n\
                            msc clean start --include-recent     # Delete all files (dangerous!)\n\
                            msc clean start --include-recycle    # Include Recycle Bin in cleanup"
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
                                .help("Include recently modified files (dangerous!)")
                                .action(clap::ArgAction::SetTrue)
                                .conflicts_with("min-age"),
                        )
                        .arg(
                            Arg::new("include-recycle")
                                .long("include-recycle")
                                .help("Include Recycle Bin in cleanup")
                                .action(clap::ArgAction::SetTrue),
                        ),
                )
                .subcommand(
                    Command::new("add")
                        .about("Add a custom clean path")
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
                                .help("Skip safety warnings (dangerous!)")
                                .action(clap::ArgAction::SetTrue),
                        ),
                )
                .subcommand(Command::new("list").about("List all clean paths (default and custom)"))
                .subcommand(Command::new("remove").about("Remove a clean path (interactive)"))
                .subcommand(
                    Command::new("reset").about("Reset clean paths just keeps default paths"),
                ),
        )
}
