use std::env;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();

    // We expect `mock-rtk rewrite <command>`
    if args.len() >= 3 && args[1] == "rewrite" {
        let command = &args[2];
        match command.as_str() {
            "ls" => {
                print!("ls -la");
                exit(0);
            }
            "fail" => {
                eprintln!("mock simulated rtk error");
                exit(1);
            }
            "empty" => {
                // Print nothing, exit success
                exit(0);
            }
            "passthrough_code_3" => {
                // code 3 means RTK decided not to rewrite, and printed nothing or we just ignore it
                eprintln!("rtk bypassed");
                exit(3);
            }
            _ => {
                eprintln!("unknown mock command: {}", command);
                exit(1);
            }
        }
    }

    eprintln!("Invalid mock arguments");
    exit(1);
}
