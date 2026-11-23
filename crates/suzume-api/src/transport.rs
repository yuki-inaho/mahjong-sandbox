//! Bounded native JSON-lines transport. The engine remains independent of stdin/stdout.
use std::io::{self, BufRead, Write};

use crate::{
    protocol::Response,
    types::{ErrorCode, Version},
    Engine, MAX_REQUEST_BYTES,
};

fn read_request(input: &mut impl BufRead) -> io::Result<Option<Result<String, &'static str>>> {
    let mut bytes = Vec::new();
    let mut oversized = false;
    let mut received = false;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            if !received {
                return Ok(None);
            }
            break;
        }
        received = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let length = newline.unwrap_or(available.len());
        if !oversized {
            if bytes.len().saturating_add(length) > MAX_REQUEST_BYTES {
                oversized = true;
                bytes.clear();
            } else {
                bytes.extend_from_slice(&available[..length]);
            }
        }
        input.consume(length + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if oversized {
        return Ok(Some(Err("request exceeds 1048576 bytes")));
    }
    Ok(Some(
        String::from_utf8(bytes).map_err(|_| "request is not UTF-8"),
    ))
}

/// One request line produces one response line; invalid lines do not desynchronize later requests.
pub fn serve(input: &mut impl BufRead, output: &mut impl Write) -> io::Result<()> {
    let mut engine = Engine::new();
    while let Some(request) = read_request(input)? {
        let response = match request {
            Ok(text) => engine.request(&text),
            Err(message) => serde_json::to_string(&Response::Error {
                version: Version,
                code: ErrorCode::InvalidRequest,
                message: message.into(),
                session_id: None,
                revision: None,
            })
            .expect("static transport error is JSON serializable"),
        };
        output.write_all(response.as_bytes())?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}
