use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "numlang", about = "numlang compiler toolchain")]
pub struct Cli {
    #[arg(short, long)]
    pub emit_tokens: bool,

    #[arg(short, long)]
    pub emit_ast: bool,

    pub file: Option<std::path::PathBuf>,
}

fn main() {
    let _cli = Cli::parse();
    println!("numlang v0.1.0");
}
