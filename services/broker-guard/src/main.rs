use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: broker-guard <action> [args...]");
        process::exit(1);
    }

    let action = &args[1];
    
    // Fail-closed security rule for any action that is a write/place order:
    match action.as_str() {
        "place_order" | "modify_order" | "cancel_order" | "gtt" | "order" => {
            eprintln!("SECURITY ERROR: broker-guard blocked illegal order action '{}'. Trading is READ-ONLY for the agent loop.", action);
            process::exit(1);
        }
        "get_positions" | "get_holdings" | "get_margins" | "read" => {
            println!("broker-guard: Approved read action '{}'.", action);
            process::exit(0);
        }
        _ => {
            eprintln!("SECURITY ERROR: broker-guard blocked unknown action '{}'. Failing closed.", action);
            process::exit(1);
        }
    }
}
