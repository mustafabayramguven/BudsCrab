//! budscrab CLI
//!
//!   budscrab decode "fd 31 10 61 ..."     decode hex frames
//!   budscrab scan btsnoop_hci.log         find every Buds frame in a capture
//!   budscrab encode 120 01                build a frame (id, payload hex)
use budscrab_proto::{ids, scan, Frame, Incoming};
use std::{env, fs, process::exit};

fn parse_hex(s: &str) -> Vec<u8> {
    s.split(|c: char| c.is_whitespace() || c == ',' || c == ':')
        .filter(|t| !t.is_empty())
        .map(|t| u8::from_str_radix(t.trim_start_matches("0x"), 16).unwrap_or_else(|_| {
            eprintln!("bad hex byte: {t}");
            exit(2)
        }))
        .collect()
}

fn print(off: usize, f: &Frame) {
    let hex: Vec<String> = f.payload.iter().map(|b| format!("{b:02X}")).collect();
    println!("@{off:<8} {:<34} id={:<3} len={:<3} [{}]", ids::name(f.id), f.id, f.payload.len(), hex.join(" "));
    println!("          {:?}", Incoming::parse(f));
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("decode") => {
            let bytes = parse_hex(&args[1..].join(" "));
            for (off, f) in scan(&bytes) {
                print(off, &f);
            }
        }
        Some("scan") => {
            let path = args.get(1).unwrap_or_else(|| { eprintln!("usage: budscrab scan <file>"); exit(2) });
            let blob = fs::read(path).unwrap_or_else(|e| { eprintln!("{path}: {e}"); exit(1) });
            let hits = scan(&blob);
            for (off, f) in &hits {
                print(*off, f);
            }
            eprintln!("{} frame(s) found", hits.len());
        }
        Some("encode") => {
            let id: u8 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| { eprintln!("usage: budscrab encode <id> [payload hex]"); exit(2) });
            let payload = parse_hex(&args[2..].join(" "));
            let raw = Frame::request(id, payload).encode();
            println!("{}", raw.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "));
        }
        _ => {
            eprintln!("budscrab decode <hex...> | scan <file> | encode <id> [payload hex]");
            exit(2);
        }
    }
}
