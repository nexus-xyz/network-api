use colored::Colorize;

pub const LOGO_NAME: &str = r#"
  ███╗   ██╗  ███████╗  ██╗  ██╗  ██╗   ██╗  ███████╗
  ████╗  ██║  ██╔════╝  ╚██╗██╔╝  ██║   ██║  ██╔════╝
  ██╔██╗ ██║  █████╗     ╚███╔╝   ██║   ██║  ███████╗
  ██║╚██╗██║  ██╔══╝     ██╔██╗   ██║   ██║  ╚════██║
  ██║ ╚████║  ███████╗  ██╔╝ ██╗  ╚██████╔╝  ███████║
  ╚═╝  ╚═══╝  ╚══════╝  ╚═╝  ╚═╝   ╚═════╝   ╚══════╝
"#;

pub fn print_banner() {
    // Split the logo into lines and color them differently
    let logo_lines: Vec<&str> = LOGO_NAME.lines().collect();
    for line in logo_lines {
        let mut colored_line = String::new();
        for c in line.chars() {
            if c == '█' {
                colored_line.push_str(&format!("{}", "█".bright_white()));
            } else {
                colored_line.push_str(&format!("{}", c.to_string().cyan()));
            }
        }
        println!("{}", colored_line);
    }
    
    let version = match option_env!("CARGO_PKG_VERSION") {
        Some(v) => format!("v{}", v),
        None => "(unknown version)".into(),
    };
    println!(
        "{} {} {}\n",
        "Welcome to the".bright_white(),
        "Nexus Network CLI".bright_cyan().bold(),
        version.bright_white()
    );
    println!(
        "{}",
        "The Nexus network is a massively-parallelized proof network for executing and proving the \x1b]8;;https://docs.nexus.org\x1b\\Nexus zkVM\x1b]8;;\x1b\\."
            .bright_white()
    );
}
