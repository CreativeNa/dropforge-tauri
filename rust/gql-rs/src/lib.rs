//! JS bindings for `src/core/gql.js`: generated from the signatures in `port.rs` by rust-anything.
//! Regenerate with `rust-anything refresh` after changing them.

use napi_derive::napi;

pub mod js;
pub mod port;
pub mod wire;

#[allow(unused_imports)]
use serde_json::Value;
pub mod utf16;

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into())
}

#[allow(dead_code)]
fn run<T>(f: impl FnOnce() -> Result<T, String>) -> napi::Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => result.map_err(napi::Error::from_reason),
        Err(payload) => Err(napi::Error::from_reason(format!(
            "Rust panic: {}",
            panic_message(&*payload)
        ))),
    }
}

/// `operationBody`.
#[napi(js_name = "operationBody")]
pub fn operation_body(name: String, variables: String, ops: String) -> napi::Result<String> {
    run(move || {
        let name = crate::js::from_json(&name)?;
        let variables = crate::js::from_json(&variables)?;
        let ops = crate::js::from_json(&ops)?;
        let out = port::operation_body(name, variables, ops)?;
        crate::js::to_json(&out)
    })
}

/// `buildBatchBody`.
#[napi(js_name = "buildBatchBody")]
pub fn build_batch_body(operations: String, ops: String) -> napi::Result<String> {
    run(move || {
        let operations = crate::js::from_json(&operations)?;
        let ops = crate::js::from_json(&ops)?;
        let out = port::build_batch_body(operations, ops)?;
        crate::js::to_json(&out)
    })
}

/// `interpretBatchPayload`.
#[napi(js_name = "interpretBatchPayload")]
pub fn interpret_batch_payload(
    payload: String,
    body: String,
    operations: String,
    arg3: String,
) -> napi::Result<String> {
    run(move || {
        let payload = crate::js::from_json(&payload)?;
        let body = crate::js::from_json(&body)?;
        let operations = crate::js::from_json(&operations)?;
        let arg3 = crate::js::from_json(&arg3)?;
        let out = port::interpret_batch_payload(payload, body, operations, arg3)?;
        crate::js::to_json(&out)
    })
}

// ---- the binary wire ------------------------------------------------------------------------

thread_local! {
    static WIRE: std::cell::RefCell<Option<(napi::bindgen_prelude::Uint8Array, napi::bindgen_prelude::Uint8Array)>> =
        const { std::cell::RefCell::new(None) };
    static PENDING: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Registers the argument and result buffers `index.cjs` writes to and reads from.
#[napi(js_name = "rustAnythingWireInit")]
pub fn rust_anything_wire_init(
    input: napi::bindgen_prelude::Uint8Array,
    output: napi::bindgen_prelude::Uint8Array,
) {
    WIRE.with(|w| *w.borrow_mut() = Some((input, output)));
}

/// Copies a result that did not fit into the (now larger) result buffer.
#[napi(js_name = "rustAnythingWireFlush")]
pub fn rust_anything_wire_flush() -> f64 {
    let bytes = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
    WIRE.with(|w| {
        let mut w = w.borrow_mut();
        let Some((_, output)) = w.as_mut() else {
            return -1.0;
        };
        // SAFETY: the call is synchronous; JS does not touch `output` until it returns.
        let out = unsafe { output.as_mut() };
        let n = bytes.len().min(out.len());
        out[..n].copy_from_slice(&bytes[..n]);
        n as f64
    })
}

/// Reads `len` argument bytes, runs `f`, and writes its result; a negative return is the size
/// of a result that did not fit, kept for `rustAnythingWireFlush`.
fn wire_io(
    len: u32,
    f: impl FnOnce(&mut wire::Reader, &mut wire::Writer) -> Result<(), String>,
) -> napi::Result<f64> {
    WIRE.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some((input, output)) = guard.as_mut() else {
            return Err(napi::Error::from_reason(
                "rust-anything: the wire buffers are not registered",
            ));
        };
        let len = (len as usize).min(input.len());
        let mut w = wire::Writer::default();
        match run(|| f(&mut wire::Reader::new(&input[..len]), &mut w)) {
            // A result of another shape than the TS type: JS runs the call through JSON.
            Err(e) if e.reason.starts_with(wire::MISMATCH) => return Ok(f64::NAN),
            other => other?,
        }
        // SAFETY: the call is synchronous; JS does not touch `output` until it returns.
        let out = unsafe { output.as_mut() };
        let n = w.0.len();
        if n > out.len() {
            PENDING.with(|p| *p.borrow_mut() = w.0);
            return Ok(-(n as f64));
        }
        out[..n].copy_from_slice(&w.0);
        Ok(n as f64)
    })
}

static S_OPERATION_BODY_0: wire::Schema = wire::Schema::Dyn;
static S_OPERATION_BODY_OUT: wire::Schema = wire::Schema::Dyn;

/// `operationBody` over the binary wire.
#[napi(js_name = "operationBodyWire")]
pub fn operation_body_wire(len: u32) -> napi::Result<f64> {
    wire_io(len, |r, w| {
        let name: serde_json::Value = wire::read_value(r, &S_OPERATION_BODY_0)?;
        let variables: Option<serde_json::Value> =
            serde_json::from_value(wire::read_dyn(r)?).map_err(|e| format!("TypeError: {e}"))?;
        let ops: Option<serde_json::Value> =
            serde_json::from_value(wire::read_dyn(r)?).map_err(|e| format!("TypeError: {e}"))?;
        let out = &port::operation_body(name, variables, ops)?;
        wire::write_value(w, out, &S_OPERATION_BODY_OUT)?;
        Ok(())
    })
}

static S_BUILD_BATCH_BODY_0: wire::Schema = wire::Schema::Dyn;
static S_BUILD_BATCH_BODY_OUT: wire::Schema = wire::Schema::Dyn;

/// `buildBatchBody` over the binary wire.
#[napi(js_name = "buildBatchBodyWire")]
pub fn build_batch_body_wire(len: u32) -> napi::Result<f64> {
    wire_io(len, |r, w| {
        let operations: serde_json::Value = wire::read_value(r, &S_BUILD_BATCH_BODY_0)?;
        let ops: Option<serde_json::Value> =
            serde_json::from_value(wire::read_dyn(r)?).map_err(|e| format!("TypeError: {e}"))?;
        let out = &port::build_batch_body(operations, ops)?;
        wire::write_value(w, out, &S_BUILD_BATCH_BODY_OUT)?;
        Ok(())
    })
}

static S_INTERPRET_BATCH_PAYLOAD_0: wire::Schema = wire::Schema::Dyn;
static S_INTERPRET_BATCH_PAYLOAD_1: wire::Schema = wire::Schema::Dyn;
static S_INTERPRET_BATCH_PAYLOAD_2: wire::Schema = wire::Schema::Dyn;
static S_INTERPRET_BATCH_PAYLOAD_OUT: wire::Schema = wire::Schema::Dyn;

/// `interpretBatchPayload` over the binary wire.
#[napi(js_name = "interpretBatchPayloadWire")]
pub fn interpret_batch_payload_wire(len: u32) -> napi::Result<f64> {
    wire_io(len, |r, w| {
        let payload: serde_json::Value = wire::read_value(r, &S_INTERPRET_BATCH_PAYLOAD_0)?;
        let body: serde_json::Value = wire::read_value(r, &S_INTERPRET_BATCH_PAYLOAD_1)?;
        let operations: serde_json::Value = wire::read_value(r, &S_INTERPRET_BATCH_PAYLOAD_2)?;
        let arg3: Option<serde_json::Value> =
            serde_json::from_value(wire::read_dyn(r)?).map_err(|e| format!("TypeError: {e}"))?;
        let out = &port::interpret_batch_payload(payload, body, operations, arg3)?;
        wire::write_value(w, out, &S_INTERPRET_BATCH_PAYLOAD_OUT)?;
        Ok(())
    })
}
