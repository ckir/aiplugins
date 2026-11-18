// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 together with the
// additional terms (evaluation grant, anti-circumvention) that apply to this
// repository. See the LICENSE file at the repository root.

use std::env;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();

    // We expect `mock-rtk rewrite <command>`
    if args.len() >= 4 && args[1] == "rewrite" && args[2] == "--" {
        let command = &args[3];
        match command.as_str() {
            "ls" => {
                print!("ls -la");
                exit(0);
            }
            "-r" => {
                print!("rtk -r");
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
            // Exit 3 is a *success* code for rtk, equivalent to 0: the rewritten
            // command is on stdout. The two cases below are deliberately split,
            // because collapsing them is what previously hid the special case —
            // a code-3 exit with no output returns None through the
            // empty-stdout branch, so it looks identical to a code-0 exit with
            // no output and proves nothing about code 3 at all.
            "code_3_with_rewrite" => {
                print!("rtk rewritten-on-code-3");
                exit(3);
            }
            "code_3_no_output" => {
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
