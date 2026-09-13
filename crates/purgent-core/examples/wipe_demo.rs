use std::path::PathBuf;

use purgent_core::modules::config::WipeStandard;
use purgent_core::modules::drive_eraser::{wipe, WipeRequest, WipeTarget};

static STANDARD_HINT: &str = "nist_clear | nist_purge | dod_3pass";

fn parse_standard(input: &str) -> Option<WipeStandard> {
    match input.trim().to_ascii_lowercase().as_str() {
        "nist_clear" | "clear" => Some(WipeStandard::Nist800_88Clear),
        "nist_purge" | "purge" => Some(WipeStandard::Nist800_88Purge),
        "dod_3pass" | "dod" => Some(WipeStandard::Dod522022M),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!(
            "usage: {} <image-file> <{}> <operator-id>",
            args.get(0).unwrap_or(&"wipe_demo".into()),
            STANDARD_HINT
        );
        std::process::exit(2);
    }

    let image = PathBuf::from(&args[1]);
    let standard = match parse_standard(&args[2]) {
        Some(s) => s,
        None => {
            eprintln!("unknown standard '{}'; expected {STANDARD_HINT}", args[2]);
            std::process::exit(2);
        }
    };
    let operator_id = args[3].clone();

    if !image.is_file() {
        eprintln!(
            "error: '{}' is not an existing file. Image-file targets only.",
            image.display()
        );
        std::process::exit(2);
    }

    let target = WipeTarget::ImageFile(image.clone());
    let description = target.display();
    eprintln!("target detected as: {description}");
    eprintln!("type the exact target description above to confirm the wipe, or abort:");
    let mut typed = String::new();
    use std::io::Write as _;
    std::io::stdout().flush().unwrap();
    std::io::stdin()
        .read_line(&mut typed)
        .expect("read confirmation");
    let typed = typed.trim().to_string();

    match wipe(WipeRequest {
        operator_id,
        target,
        standard,
        operator_confirmed_target: typed,
        block_size: None,
    }) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("serialize")
            );
            if !result.complete {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("wipe failed: {e:?}");
            std::process::exit(1);
        }
    }
}
