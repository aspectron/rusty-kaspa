use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use kaspa_wasm_core::types::{BinaryT, HexString};

use crate::imports::*;
use crate::result::Result;
use kaspa_txscript::script_builder as native;

///
///  ScriptBuilder provides a facility for building custom scripts. It allows
/// you to push opcodes, ints, and data while respecting canonical encoding. In
/// general it does not ensure the script will execute correctly, however any
/// data pushes which would exceed the maximum allowed script engine limits and
/// are therefore guaranteed not to execute will not be pushed and will result in
/// the Script function returning an error.
///
/// @see {@link Opcode}
/// @category Consensus
#[derive(Clone)]
#[wasm_bindgen(inspectable)]
pub struct ScriptBuilder {
    script_builder: Rc<RefCell<native::ScriptBuilder>>,
}

impl ScriptBuilder {
    #[inline]
    pub fn inner(&self) -> Ref<'_, native::ScriptBuilder> {
        self.script_builder.borrow()
    }

    #[inline]
    pub fn inner_mut(&self) -> RefMut<'_, native::ScriptBuilder> {
        self.script_builder.borrow_mut()
    }
}

impl Default for ScriptBuilder {
    fn default() -> Self {
        Self { script_builder: Rc::new(RefCell::new(kaspa_txscript::script_builder::ScriptBuilder::new())) }
    }
}

#[wasm_bindgen]
impl ScriptBuilder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen(getter)]
    pub fn data(&self) -> HexString {
        self.script()
    }

    /// Get script bytes represented by a hex string.
    pub fn script(&self) -> HexString {
        let inner = self.inner();
        HexString::from(inner.script())
    }

    /// Drains (empties) the script builder, returning the
    /// script bytes represented by a hex string.
    pub fn drain(&self) -> HexString {
        let mut inner = self.inner_mut();
        HexString::from(inner.drain().as_slice())
    }

    #[wasm_bindgen(js_name = canonicalDataSize)]
    pub fn canonical_data_size(data: BinaryT) -> Result<u32> {
        let data = data.try_as_vec_u8()?;
        let size = native::ScriptBuilder::canonical_data_size(&data) as u32;
        Ok(size)
    }

    /// Pushes the passed opcode to the end of the script. The script will not
    /// be modified if pushing the opcode would cause the script to exceed the
    /// maximum allowed script engine size.
    #[wasm_bindgen(js_name = addOp)]
    pub fn add_op(&self, op: u8) -> Result<ScriptBuilder> {
        let mut inner = self.inner_mut();
        inner.add_op(op)?;
        Ok(self.clone())
    }

    /// Adds the passed opcodes to the end of the script.
    /// Supplied opcodes can be represented as a `Uint8Array` or a `HexString`.
    #[wasm_bindgen(js_name = "addOps")]
    pub fn add_ops(&self, opcodes: JsValue) -> Result<ScriptBuilder> {
        let opcodes = opcodes.try_as_vec_u8()?;
        self.inner_mut().add_ops(&opcodes)?;
        Ok(self.clone())
    }

    /// AddData pushes the passed data to the end of the script. It automatically
    /// chooses canonical opcodes depending on the length of the data.
    ///
    /// A zero length buffer will lead to a push of empty data onto the stack (Op0 = OpFalse)
    /// and any push of data greater than [`MAX_SCRIPT_ELEMENT_SIZE`](kaspa_txscript::MAX_SCRIPT_ELEMENT_SIZE) will not modify
    /// the script since that is not allowed by the script engine.
    ///
    /// Also, the script will not be modified if pushing the data would cause the script to
    /// exceed the maximum allowed script engine size [`MAX_SCRIPTS_SIZE`](kaspa_txscript::MAX_SCRIPTS_SIZE).
    #[wasm_bindgen(js_name = addData)]
    pub fn add_data(&self, data: BinaryT) -> Result<ScriptBuilder> {
        let data = data.try_as_vec_u8()?;

        let mut inner = self.inner_mut();
        inner.add_data(&data)?;
        Ok(self.clone())
    }

    #[wasm_bindgen(js_name = addI64)]
    pub fn add_i64(&self, value: i64) -> Result<ScriptBuilder> {
        let mut inner = self.inner_mut();
        inner.add_i64(value)?;
        Ok(self.clone())
    }

    #[wasm_bindgen(js_name = addLockTime)]
    pub fn add_lock_time(&self, lock_time: u64) -> Result<ScriptBuilder> {
        let mut inner = self.inner_mut();
        inner.add_lock_time(lock_time)?;
        Ok(self.clone())
    }

    #[wasm_bindgen(js_name = addSequence)]
    pub fn add_sequence(&self, sequence: u64) -> Result<ScriptBuilder> {
        let mut inner = self.inner_mut();
        inner.add_sequence(sequence)?;
        Ok(self.clone())
    }
}
