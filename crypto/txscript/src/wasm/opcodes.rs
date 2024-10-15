#[cfg(feature = "py-sdk")]
use pyo3::prelude::*;
pub use wasm_bindgen::prelude::*;

/// Kaspa Transaction Script Opcodes
/// @see {@link ScriptBuilder}
/// @category Consensus
#[derive(Clone)]
#[cfg_attr(feature = "py-sdk", pyclass)]
#[wasm_bindgen]
pub enum Opcodes {
    OpFalse = 0x00,

    OpData1 = 0x01,
    OpData2 = 0x02,
    OpData3 = 0x03,
    OpData4 = 0x04,
    OpData5 = 0x05,
    OpData6 = 0x06,
    OpData7 = 0x07,
    OpData8 = 0x08,
    OpData9 = 0x09,
    OpData10 = 0x0a,
    OpData11 = 0x0b,
    OpData12 = 0x0c,
    OpData13 = 0x0d,
    OpData14 = 0x0e,
    OpData15 = 0x0f,
    OpData16 = 0x10,
    OpData17 = 0x11,
    OpData18 = 0x12,
    OpData19 = 0x13,
    OpData20 = 0x14,
    OpData21 = 0x15,
    OpData22 = 0x16,
    OpData23 = 0x17,
    OpData24 = 0x18,
    OpData25 = 0x19,
    OpData26 = 0x1a,
    OpData27 = 0x1b,
    OpData28 = 0x1c,
    OpData29 = 0x1d,
    OpData30 = 0x1e,
    OpData31 = 0x1f,
    OpData32 = 0x20,
    OpData33 = 0x21,
    OpData34 = 0x22,
    OpData35 = 0x23,
    OpData36 = 0x24,
    OpData37 = 0x25,
    OpData38 = 0x26,
    OpData39 = 0x27,
    OpData40 = 0x28,
    OpData41 = 0x29,
    OpData42 = 0x2a,
    OpData43 = 0x2b,
    OpData44 = 0x2c,
    OpData45 = 0x2d,
    OpData46 = 0x2e,
    OpData47 = 0x2f,
    OpData48 = 0x30,
    OpData49 = 0x31,
    OpData50 = 0x32,
    OpData51 = 0x33,
    OpData52 = 0x34,
    OpData53 = 0x35,
    OpData54 = 0x36,
    OpData55 = 0x37,
    OpData56 = 0x38,
    OpData57 = 0x39,
    OpData58 = 0x3a,
    OpData59 = 0x3b,
    OpData60 = 0x3c,
    OpData61 = 0x3d,
    OpData62 = 0x3e,
    OpData63 = 0x3f,
    OpData64 = 0x40,
    OpData65 = 0x41,
    OpData66 = 0x42,
    OpData67 = 0x43,
    OpData68 = 0x44,
    OpData69 = 0x45,
    OpData70 = 0x46,
    OpData71 = 0x47,
    OpData72 = 0x48,
    OpData73 = 0x49,
    OpData74 = 0x4a,
    OpData75 = 0x4b,

    OpPushData1 = 0x4c,
    OpPushData2 = 0x4d,
    OpPushData4 = 0x4e,

    Op1Negate = 0x4f,

    OpReserved = 0x50,

    OpTrue = 0x51,

    Op2 = 0x52,
    Op3 = 0x53,
    Op4 = 0x54,
    Op5 = 0x55,
    Op6 = 0x56,
    Op7 = 0x57,
    Op8 = 0x58,
    Op9 = 0x59,
    Op10 = 0x5a,
    Op11 = 0x5b,
    Op12 = 0x5c,
    Op13 = 0x5d,
    Op14 = 0x5e,
    Op15 = 0x5f,
    Op16 = 0x60,

    OpNop = 0x61,
    OpVer = 0x62,
    OpIf = 0x63,
    OpNotIf = 0x64,
    OpVerIf = 0x65,
    OpVerNotIf = 0x66,

    OpElse = 0x67,
    OpEndIf = 0x68,
    OpVerify = 0x69,
    OpReturn = 0x6a,
    OpToAltStack = 0x6b,
    OpFromAltStack = 0x6c,

    Op2Drop = 0x6d,
    Op2Dup = 0x6e,
    Op3Dup = 0x6f,
    Op2Over = 0x70,
    Op2Rot = 0x71,
    Op2Swap = 0x72,
    OpIfDup = 0x73,
    OpDepth = 0x74,
    OpDrop = 0x75,
    OpDup = 0x76,
    OpNip = 0x77,
    OpOver = 0x78,
    OpPick = 0x79,

    OpRoll = 0x7a,
    OpRot = 0x7b,
    OpSwap = 0x7c,
    OpTuck = 0x7d,

    /// Splice opcodes.
    OpCat = 0x7e,
    OpSubStr = 0x7f,
    OpLeft = 0x80,
    OpRight = 0x81,

    OpSize = 0x82,

    /// Bitwise logic opcodes.
    OpInvert = 0x83,
    OpAnd = 0x84,
    OpOr = 0x85,
    OpXor = 0x86,

    OpEqual = 0x87,
    OpEqualVerify = 0x88,

    OpReserved1 = 0x89,
    OpReserved2 = 0x8a,

    /// Numeric related opcodes.
    Op1Add = 0x8b,
    Op1Sub = 0x8c,
    Op2Mul = 0x8d,
    Op2Div = 0x8e,
    OpNegate = 0x8f,
    OpAbs = 0x90,
    OpNot = 0x91,
    Op0NotEqual = 0x92,

    OpAdd = 0x93,
    OpSub = 0x94,
    OpMul = 0x95,
    OpDiv = 0x96,
    OpMod = 0x97,
    OpLShift = 0x98,
    OpRShift = 0x99,

    OpBoolAnd = 0x9a,
    OpBoolOr = 0x9b,

    OpNumEqual = 0x9c,
    OpNumEqualVerify = 0x9d,
    OpNumNotEqual = 0x9e,

    OpLessThan = 0x9f,
    OpGreaterThan = 0xa0,
    OpLessThanOrEqual = 0xa1,
    OpGreaterThanOrEqual = 0xa2,
    OpMin = 0xa3,
    OpMax = 0xa4,
    OpWithin = 0xa5,

    /// Undefined opcodes.
    OpUnknown166 = 0xa6,
    OpUnknown167 = 0xa7,

    /// Crypto opcodes.
    OpSHA256 = 0xa8,

    OpCheckMultiSigECDSA = 0xa9,

    OpBlake2b = 0xaa,
    OpCheckSigECDSA = 0xab,
    OpCheckSig = 0xac,
    OpCheckSigVerify = 0xad,
    OpCheckMultiSig = 0xae,
    OpCheckMultiSigVerify = 0xaf,
    OpCheckLockTimeVerify = 0xb0,
    OpCheckSequenceVerify = 0xb1,

    /// Undefined opcodes.
    OpUnknown178 = 0xb2,
    OpUnknown179 = 0xb3,
    OpUnknown180 = 0xb4,
    OpUnknown181 = 0xb5,
    OpUnknown182 = 0xb6,
    OpUnknown183 = 0xb7,
    OpUnknown184 = 0xb8,
    OpUnknown185 = 0xb9,
    OpUnknown186 = 0xba,
    OpUnknown187 = 0xbb,
    OpUnknown188 = 0xbc,
    OpUnknown189 = 0xbd,
    OpUnknown190 = 0xbe,
    OpUnknown191 = 0xbf,
    OpUnknown192 = 0xc0,
    OpUnknown193 = 0xc1,
    OpUnknown194 = 0xc2,
    OpUnknown195 = 0xc3,
    OpUnknown196 = 0xc4,
    OpUnknown197 = 0xc5,
    OpUnknown198 = 0xc6,
    OpUnknown199 = 0xc7,
    OpUnknown200 = 0xc8,
    OpUnknown201 = 0xc9,
    OpUnknown202 = 0xca,
    OpUnknown203 = 0xcb,
    OpUnknown204 = 0xcc,
    OpUnknown205 = 0xcd,
    OpUnknown206 = 0xce,
    OpUnknown207 = 0xcf,
    OpUnknown208 = 0xd0,
    OpUnknown209 = 0xd1,
    OpUnknown210 = 0xd2,
    OpUnknown211 = 0xd3,
    OpUnknown212 = 0xd4,
    OpUnknown213 = 0xd5,
    OpUnknown214 = 0xd6,
    OpUnknown215 = 0xd7,
    OpUnknown216 = 0xd8,
    OpUnknown217 = 0xd9,
    OpUnknown218 = 0xda,
    OpUnknown219 = 0xdb,
    OpUnknown220 = 0xdc,
    OpUnknown221 = 0xdd,
    OpUnknown222 = 0xde,
    OpUnknown223 = 0xdf,
    OpUnknown224 = 0xe0,
    OpUnknown225 = 0xe1,
    OpUnknown226 = 0xe2,
    OpUnknown227 = 0xe3,
    OpUnknown228 = 0xe4,
    OpUnknown229 = 0xe5,
    OpUnknown230 = 0xe6,
    OpUnknown231 = 0xe7,
    OpUnknown232 = 0xe8,
    OpUnknown233 = 0xe9,
    OpUnknown234 = 0xea,
    OpUnknown235 = 0xeb,
    OpUnknown236 = 0xec,
    OpUnknown237 = 0xed,
    OpUnknown238 = 0xee,
    OpUnknown239 = 0xef,
    OpUnknown240 = 0xf0,
    OpUnknown241 = 0xf1,
    OpUnknown242 = 0xf2,
    OpUnknown243 = 0xf3,
    OpUnknown244 = 0xf4,
    OpUnknown245 = 0xf5,
    OpUnknown246 = 0xf6,
    OpUnknown247 = 0xf7,
    OpUnknown248 = 0xf8,
    OpUnknown249 = 0xf9,

    OpSmallInteger = 0xfa,
    OpPubKeys = 0xfb,
    OpUnknown252 = 0xfc,
    OpPubKeyHash = 0xfd,
    OpPubKey = 0xfe,
    OpInvalidOpCode = 0xff,
}

#[cfg(feature = "py-sdk")]
#[pymethods]
impl Opcodes {
    #[getter]
    pub fn value(&self) -> u8 {
        self.clone() as u8
    }
}
