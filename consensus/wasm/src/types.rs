//!
//! General-purpose types for WASM bindings
//!

use wasm_bindgen::prelude::*;

// export type HexString = string;

#[wasm_bindgen(typescript_custom_section)]
const TS_HEX_STRING: &'static str = r#"
/**
 * A string containing a hexadecimal representation of the data (typically representing for IDs or Hashes).
 * 
 * @category General
 */ 
export type HexString = string;
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "HexString")]
    pub type HexString;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Array<string>")]
    pub type StringArray;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Array<string> | undefined")]
    pub type StringArrayOrNone;
}