use std::{collections::BTreeMap, env, process};

use attestation_types::{digest_to_hex, GateContext, ProgramOwner};

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };

    match command {
        "version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "context-hash" => {
            let options = parse_options(&args[1..])?;
            let context = GateContext {
                application_id: required(&options, "application-id")?,
                gate_id: required(&options, "gate-id")?,
                token_program_owner: parse_program_owner(&required(&options, "token-owner-hex")?)?,
                threshold: required(&options, "threshold")?
                    .parse()
                    .map_err(|_| "--threshold must be a u128 integer".to_owned())?,
                verifier_id: required(&options, "verifier-id")?,
                expires_at_unix_ms: optional(&options, "expires-at-unix-ms")?
                    .map(|value| {
                        value
                            .parse()
                            .map_err(|_| "--expires-at-unix-ms must be a u64 integer".to_owned())
                    })
                    .transpose()?,
            };

            println!("{}", digest_to_hex(&context.context_hash()));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        unknown => Err(format!("unknown command `{unknown}`")),
    }
}

fn print_help() {
    println!(
        "TokenStudio ProofGate\n\n\
         Commands:\n\
           proofgate version\n\
           proofgate context-hash \\\n\
             --application-id <id> \\\n\
             --gate-id <id> \\\n\
             --token-owner-hex <32-byte-hex> \\\n\
             --threshold <u128> \\\n\
             --verifier-id <id> \\\n\
             [--expires-at-unix-ms <u64>]\n"
    );
}

fn parse_options(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut options = BTreeMap::new();
    let mut index = 0;

    while index < args.len() {
        let key = args[index]
            .strip_prefix("--")
            .ok_or_else(|| format!("expected option starting with --, got `{}`", args[index]))?;
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for --{key}"))?;
        if value.starts_with("--") {
            return Err(format!("missing value for --{key}"));
        }
        options.insert(key.to_owned(), value.to_owned());
        index += 2;
    }

    Ok(options)
}

fn required(options: &BTreeMap<String, String>, key: &str) -> Result<String, String> {
    options
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing --{key}"))
}

fn optional(options: &BTreeMap<String, String>, key: &str) -> Result<Option<String>, String> {
    Ok(options.get(key).cloned())
}

fn parse_program_owner(hex: &str) -> Result<ProgramOwner, String> {
    let bytes = parse_hex_32(hex)?;
    let mut owner = [0_u32; 8];
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        owner[index] = u32::from_le_bytes(chunk.try_into().expect("chunk has four bytes"));
    }
    Ok(owner)
}

fn parse_hex_32(hex: &str) -> Result<[u8; 32], String> {
    let hex = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    if hex.len() != 64 {
        return Err("--token-owner-hex must be exactly 32 bytes / 64 hex chars".to_owned());
    }

    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = parse_hex_byte(&hex[offset..offset + 2])?;
    }
    Ok(bytes)
}

fn parse_hex_byte(hex: &str) -> Result<u8, String> {
    u8::from_str_radix(hex, 16).map_err(|_| format!("invalid hex byte `{hex}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_program_owner_as_le_u32_words() {
        let owner =
            parse_program_owner("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .unwrap();

        assert_eq!(
            owner,
            [
                0x0302_0100,
                0x0706_0504,
                0x0b0a_0908,
                0x0f0e_0d0c,
                0x1312_1110,
                0x1716_1514,
                0x1b1a_1918,
                0x1f1e_1d1c,
            ]
        );
    }

    #[test]
    fn context_hash_command_requires_values() {
        let error = run(vec!["context-hash".to_owned()]).unwrap_err();
        assert!(error.contains("missing --application-id"));
    }
}
