use colored::Colorize;

pub const BANNER: &str = r#"
===========================================================================
███╗   ██╗███████╗██╗  ██╗██╗   ██╗███████╗
████╗  ██║██╔════╝╚██╗██╔╝██║   ██║██╔════╝
██╔██╗ ██║█████╗   ╚███╔╝ ██║   ██║███████╗
██║╚██╗██║██╔══╝   ██╔██╗ ██║   ██║╚════██║
██║ ╚████║███████╗██╔╝ ██╗╚██████╔╝███████║
╚═╝  ╚═══╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝
===========================================================================
"#;

pub fn print_banner() {
    println!("{}", BANNER.bright_cyan());
    println!(
        "{} {}\n",
        "Welcome to the".bright_white(),
        "Nexus Network CLI".bright_cyan().bold()
    );
}

pub fn print_success(message: &str) {
    println!("✨ {}", message.green());
}

pub fn print_error(message: &str) {
    println!("❌ {}", message.red());
}
