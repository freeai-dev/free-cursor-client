use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    #[command(
        about = "Install the program\nThe program will be installed to %APPDATA%/free-cursor-client, and will be started automatically"
    )]
    Install(InstallArgs),

    #[command(
        about = "Uninstall the program\nDefault only deletes the program executable, use --full to also delete configs and logs"
    )]
    Uninstall {
        #[arg(
            long,
            default_value_t = false,
            help = "Delete all program data, including configs and logs"
        )]
        full: bool,
    },

    #[command(about = "Run the service\nDO NOT USE THIS COMMAND MANUALLY")]
    Service,

    #[command(about = "Get the status of your token")]
    Status(StatusArgs),

    #[command(about = "Generate an invitation code")]
    Invite(InviteArgs),

    #[command(about = "Order a new cursor")]
    Order,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    #[arg(
        long,
        help = "The token to use. The cached token will be used if not provided"
    )]
    pub token: Option<String>,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(
        long,
        help = "The token to use. The cached token will be used if not provided"
    )]
    pub token: Option<String>,
}

#[derive(Debug, Args)]
pub struct InviteArgs {
    #[arg(
        long,
        help = "The token to use. The cached token will be used if not provided"
    )]
    pub token: Option<String>,
}
