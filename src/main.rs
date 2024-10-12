#![warn(clippy::pedantic)]
use std::error::Error;
use std::io::prelude::*;
use std::{env, io};

use prost::Message;

mod codegen;
mod ident;

// Include the `items` module, which is generated from items.proto.
// It is important to maintain the same structure as in the proto.
mod plugin {
    include!(concat!(env!("OUT_DIR"), "/plugin.rs"));
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct PluginOption {
    pub driver: String,
    pub debug: bool,
}

fn deserialize_codegen_request(buf: &[u8]) -> Result<plugin::GenerateRequest, prost::DecodeError> {
    plugin::GenerateRequest::decode(buf)
}

fn serialize_codegen_response(resp: &plugin::GenerateResponse) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut buf = Vec::with_capacity(resp.encoded_len());
    resp.encode(&mut buf)?;
    Ok(buf)
}

fn handle_option_debug(
    req: &plugin::GenerateRequest,
    resp: &mut plugin::GenerateResponse,
) -> Result<(), serde_json::error::Error> {
    let j = serde_json::to_string_pretty(req)?;
    let file = plugin::File {
        name: "plugin-request.json".to_string(),
        contents: j.as_bytes().to_vec(),
    };
    resp.files.push(file);

    Ok(())
}

fn process_request() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut buffer: Vec<u8> = Vec::new();
    _ = stdin.read_to_end(&mut buffer).unwrap();
    let req = deserialize_codegen_request(buffer.as_slice())?;

    let plugin_option: PluginOption = serde_json::from_slice(req.plugin_options.as_slice())?;

    let mut gen = codegen::Generator {
        req: req.clone(),
        structs: elsa::vec::FrozenVec::new(),
    };

    let mut resp = plugin::GenerateResponse {
        files: gen.generate(),
    };

    if plugin_option.debug {
        handle_option_debug(&req, &mut resp)?;
    }

    let out = serialize_codegen_response(&resp)?;
    io::stdout().write_all(&out)?;

    Ok(())
}

fn main() {
    match process_request() {
        Ok(()) => (),
        Err(e) => {
            eprintln!("Error: failed to process request: {e:?}");
            std::process::exit(1)
        }
    };
}
