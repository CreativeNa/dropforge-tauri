//! JS bindings for `src/core/model.js`: generated from the signatures in `port.rs` by rust-anything.
//! Regenerate with `rust-anything refresh` after changing them.

use napi_derive::napi;

pub mod js;
pub mod port;
pub mod wire;

#[allow(unused_imports)]
use serde_json::Value;
pub mod utf16;

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload.downcast_ref::<&str>().map(|s| (*s).to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".into())
}

#[allow(dead_code)]
fn run<T>(f: impl FnOnce() -> Result<T, String>) -> napi::Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => result.map_err(napi::Error::from_reason),
        Err(payload) => Err(napi::Error::from_reason(format!("Rust panic: {}", panic_message(&*payload)))),
    }
}

/// `mergeData`.
#[napi(js_name = "mergeData")]
pub fn merge_data(primary: String, secondary: String) -> napi::Result<String> {
    run(move || {
        let primary = crate::js::from_json(&primary)?;
        let secondary = crate::js::from_json(&secondary)?;
        let out = port::merge_data(primary, secondary)?;
        crate::js::to_json(&out)
    })
}

/// `campaignData`.
#[napi(js_name = "campaignData")]
pub fn campaign_data(raw: String, claimed_benefits: String) -> napi::Result<String> {
    run(move || {
        let raw = crate::js::from_json(&raw)?;
        let claimed_benefits = crate::js::from_json(&claimed_benefits)?;
        let out = port::campaign_data(raw, claimed_benefits)?;
        crate::js::to_json(&out)
    })
}

/// `campaignView`.
#[napi(js_name = "campaignView")]
pub fn campaign_view(data: String, now: String) -> napi::Result<String> {
    run(move || {
        let data = crate::js::from_json(&data)?;
        let now = crate::js::from_json(&now)?;
        let out = port::campaign_view(data, now)?;
        crate::js::to_json(&out)
    })
}

/// `groupByGame`.
#[napi(js_name = "groupByGame")]
pub fn group_by_game(campaigns: String) -> napi::Result<String> {
    run(move || {
        let campaigns = crate::js::from_json(&campaigns)?;
        let out = port::group_by_game(campaigns)?;
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
pub fn rust_anything_wire_init(input: napi::bindgen_prelude::Uint8Array, output: napi::bindgen_prelude::Uint8Array) {
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
fn wire_io(len: u32, f: impl FnOnce(&mut wire::Reader, &mut wire::Writer) -> Result<(), String>) -> napi::Result<f64> {
    WIRE.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some((input, output)) = guard.as_mut() else {
            return Err(napi::Error::from_reason("rust-anything: the wire buffers are not registered"));
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

static S_MERGE_DATA_0: wire::Schema = wire::Schema::Dyn;
static S_MERGE_DATA_1: wire::Schema = wire::Schema::Dyn;
static S_MERGE_DATA_OUT: wire::Schema = wire::Schema::Dyn;

/// `mergeData` over the binary wire.
#[napi(js_name = "mergeDataWire")]
pub fn merge_data_wire(len: u32) -> napi::Result<f64> {
    wire_io(len, |r, w| {
        let primary: serde_json::Value = wire::read_value(r, &S_MERGE_DATA_0)?;
        let secondary: serde_json::Value = wire::read_value(r, &S_MERGE_DATA_1)?;
        let out = &port::merge_data(primary, secondary)?;
        wire::write_value(w, out, &S_MERGE_DATA_OUT)?;
        Ok(())
    })
}

static S_CAMPAIGN_DATA_0: wire::Schema = wire::Schema::Dyn;
static S_CAMPAIGN_DATA_OUT: wire::Schema = wire::Schema::Dyn;

/// `campaignData` over the binary wire.
#[napi(js_name = "campaignDataWire")]
pub fn campaign_data_wire(len: u32) -> napi::Result<f64> {
    wire_io(len, |r, w| {
        let raw: serde_json::Value = wire::read_value(r, &S_CAMPAIGN_DATA_0)?;
        let claimed_benefits: Option<serde_json::Value> = serde_json::from_value(wire::read_dyn(r)?).map_err(|e| format!("TypeError: {e}"))?;
        let out = &port::campaign_data(raw, claimed_benefits)?;
        wire::write_value(w, out, &S_CAMPAIGN_DATA_OUT)?;
        Ok(())
    })
}

static S_CAMPAIGN_VIEW_0: wire::Schema = wire::Schema::Dyn;
static S_CAMPAIGN_VIEW_OUT: wire::Schema = wire::Schema::Dyn;

/// `campaignView` over the binary wire.
#[napi(js_name = "campaignViewWire")]
pub fn campaign_view_wire(len: u32) -> napi::Result<f64> {
    wire_io(len, |r, w| {
        let data: serde_json::Value = wire::read_value(r, &S_CAMPAIGN_VIEW_0)?;
        let now: Option<serde_json::Value> = serde_json::from_value(wire::read_dyn(r)?).map_err(|e| format!("TypeError: {e}"))?;
        let out = &port::campaign_view(data, now)?;
        wire::write_value(w, out, &S_CAMPAIGN_VIEW_OUT)?;
        Ok(())
    })
}

static S_GROUP_BY_GAME_0: wire::Schema = wire::Schema::Dyn;
static S_GROUP_BY_GAME_OUT: wire::Schema = wire::Schema::Dyn;

/// `groupByGame` over the binary wire.
#[napi(js_name = "groupByGameWire")]
pub fn group_by_game_wire(len: u32) -> napi::Result<f64> {
    wire_io(len, |r, w| {
        let campaigns: serde_json::Value = wire::read_value(r, &S_GROUP_BY_GAME_0)?;
        let out = &port::group_by_game(campaigns)?;
        wire::write_value(w, out, &S_GROUP_BY_GAME_OUT)?;
        Ok(())
    })
}
