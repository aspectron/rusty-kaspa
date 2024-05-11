pub use wasm_bindgen::prelude::*;

/// Kaspa Transaction Script Opcodes
/// @see {@link ScriptBuilder}
/// @category Consensus
///
#[wasm_bindgen()]
pub struct Opcode;

#[wasm_bindgen]
impl Opcode {
    /// Op0 = 0x00
    #[wasm_bindgen(getter=Op0)]
    pub fn op_0() -> u8 {
        0x00
    }

    /// OpFalse = 0x00,
    #[wasm_bindgen(getter=OpFalse)]
    pub fn op_false() -> u8 {
        0x00
    }

    /// OpData1 = 0x01
    #[wasm_bindgen(getter=OpData1)]
    pub fn op_data1() -> u8 {
        0x01
    }

    /// OpData2 = 0x02
    #[wasm_bindgen(getter=OpData2)]
    pub fn op_data2() -> u8 {
        0x02
    }
    /// OpData3 = 0x03
    #[wasm_bindgen(getter=OpData3)]
    pub fn op_data3() -> u8 {
        0x03
    }
    /// OpData4 = 0x04
    #[wasm_bindgen(getter=OpData4)]
    pub fn op_data4() -> u8 {
        0x04
    }
    /// OpData5 = 0x05
    #[wasm_bindgen(getter=OpData5)]
    pub fn op_data5() -> u8 {
        0x05
    }
    /// OpData6 = 0x06
    #[wasm_bindgen(getter=OpData6)]
    pub fn op_data6() -> u8 {
        0x06
    }
    /// OpData7 = 0x07
    #[wasm_bindgen(getter=OpData7)]
    pub fn op_data7() -> u8 {
        0x07
    }
    /// OpData8 = 0x08
    #[wasm_bindgen(getter=OpData8)]
    pub fn op_data8() -> u8 {
        0x08
    }
    /// OpData9 = 0x09
    #[wasm_bindgen(getter=OpData9)]
    pub fn op_data9() -> u8 {
        0x09
    }
    /// OpData10 = 0x0a
    #[wasm_bindgen(getter=OpData10)]
    pub fn op_data10() -> u8 {
        0x0a
    }
    /// OpData11 = 0x0b
    #[wasm_bindgen(getter=OpData11)]
    pub fn op_data11() -> u8 {
        0x0b
    }
    /// OpData12 = 0x0c
    #[wasm_bindgen(getter=OpData12)]
    pub fn op_data12() -> u8 {
        0x0c
    }
    /// OpData13 = 0x0d
    #[wasm_bindgen(getter=OpData13)]
    pub fn op_data13() -> u8 {
        0x0d
    }
    /// OpData14 = 0x0e
    #[wasm_bindgen(getter=OpData14)]
    pub fn op_data14() -> u8 {
        0x0e
    }
    /// OpData15 = 0x0f
    #[wasm_bindgen(getter=OpData15)]
    pub fn op_data15() -> u8 {
        0x0f
    }
    /// OpData16 = 0x10
    #[wasm_bindgen(getter=OpData16)]
    pub fn op_data16() -> u8 {
        0x10
    }
    /// OpData17 = 0x11
    #[wasm_bindgen(getter=OpData17)]
    pub fn op_data17() -> u8 {
        0x11
    }
    /// OpData18 = 0x12
    #[wasm_bindgen(getter=OpData18)]
    pub fn op_data18() -> u8 {
        0x12
    }
    /// OpData19 = 0x13
    #[wasm_bindgen(getter=OpData19)]
    pub fn op_data19() -> u8 {
        0x13
    }
    /// OpData20 = 0x14
    #[wasm_bindgen(getter=OpData20)]
    pub fn op_data20() -> u8 {
        0x14
    }
    /// OpData21 = 0x15
    #[wasm_bindgen(getter=OpData21)]
    pub fn op_data21() -> u8 {
        0x15
    }
    /// OpData22 = 0x16
    #[wasm_bindgen(getter=OpData22)]
    pub fn op_data22() -> u8 {
        0x16
    }
    /// OpData23 = 0x17
    #[wasm_bindgen(getter=OpData23)]
    pub fn op_data23() -> u8 {
        0x17
    }
    /// OpData24 = 0x18
    #[wasm_bindgen(getter=OpData24)]
    pub fn op_data24() -> u8 {
        0x18
    }
    /// OpData25 = 0x19
    #[wasm_bindgen(getter=OpData25)]
    pub fn op_data25() -> u8 {
        0x19
    }
    /// OpData26 = 0x1a
    #[wasm_bindgen(getter=OpData26)]
    pub fn op_data26() -> u8 {
        0x1a
    }
    /// OpData27 = 0x1b
    #[wasm_bindgen(getter=OpData27)]
    pub fn op_data27() -> u8 {
        0x1b
    }
    /// OpData28 = 0x1c
    #[wasm_bindgen(getter=OpData28)]
    pub fn op_data28() -> u8 {
        0x1c
    }
    /// OpData29 = 0x1d
    #[wasm_bindgen(getter=OpData29)]
    pub fn op_data29() -> u8 {
        0x1d
    }
    /// OpData30 = 0x1e
    #[wasm_bindgen(getter=OpData30)]
    pub fn op_data30() -> u8 {
        0x1e
    }
    /// OpData31 = 0x1f
    #[wasm_bindgen(getter=OpData31)]
    pub fn op_data31() -> u8 {
        0x1f
    }
    /// OpData32 = 0x20
    #[wasm_bindgen(getter=OpData32)]
    pub fn op_data32() -> u8 {
        0x20
    }
    /// OpData33 = 0x21
    #[wasm_bindgen(getter=OpData33)]
    pub fn op_data33() -> u8 {
        0x21
    }
    /// OpData34 = 0x22
    #[wasm_bindgen(getter=OpData34)]
    pub fn op_data34() -> u8 {
        0x22
    }
    /// OpData35 = 0x23
    #[wasm_bindgen(getter=OpData35)]
    pub fn op_data35() -> u8 {
        0x23
    }
    /// OpData36 = 0x24
    #[wasm_bindgen(getter=OpData36)]
    pub fn op_data36() -> u8 {
        0x24
    }
    /// OpData37 = 0x25
    #[wasm_bindgen(getter=OpData37)]
    pub fn op_data37() -> u8 {
        0x25
    }
    /// OpData38 = 0x26
    #[wasm_bindgen(getter=OpData38)]
    pub fn op_data38() -> u8 {
        0x26
    }
    /// OpData39 = 0x27
    #[wasm_bindgen(getter=OpData39)]
    pub fn op_data39() -> u8 {
        0x27
    }
    /// OpData40 = 0x28
    #[wasm_bindgen(getter=OpData40)]
    pub fn op_data40() -> u8 {
        0x28
    }
    /// OpData41 = 0x29
    #[wasm_bindgen(getter=OpData41)]
    pub fn op_data41() -> u8 {
        0x29
    }
    /// OpData42 = 0x2a
    #[wasm_bindgen(getter=OpData42)]
    pub fn op_data42() -> u8 {
        0x2a
    }
    /// OpData43 = 0x2b
    #[wasm_bindgen(getter=OpData43)]
    pub fn op_data43() -> u8 {
        0x2b
    }
    /// OpData44 = 0x2c
    #[wasm_bindgen(getter=OpData44)]
    pub fn op_data44() -> u8 {
        0x2c
    }
    /// OpData45 = 0x2d
    #[wasm_bindgen(getter=OpData45)]
    pub fn op_data45() -> u8 {
        0x2d
    }
    /// OpData46 = 0x2e
    #[wasm_bindgen(getter=OpData46)]
    pub fn op_data46() -> u8 {
        0x2e
    }
    /// OpData47 = 0x2f
    #[wasm_bindgen(getter=OpData47)]
    pub fn op_data47() -> u8 {
        0x2f
    }
    /// OpData48 = 0x30
    #[wasm_bindgen(getter=OpData48)]
    pub fn op_data48() -> u8 {
        0x30
    }
    /// OpData49 = 0x31
    #[wasm_bindgen(getter=OpData49)]
    pub fn op_data49() -> u8 {
        0x31
    }
    /// OpData50 = 0x32
    #[wasm_bindgen(getter=OpData50)]
    pub fn op_data50() -> u8 {
        0x32
    }
    /// OpData51 = 0x33
    #[wasm_bindgen(getter=OpData51)]
    pub fn op_data51() -> u8 {
        0x33
    }
    /// OpData52 = 0x34
    #[wasm_bindgen(getter=OpData52)]
    pub fn op_data52() -> u8 {
        0x34
    }
    /// OpData53 = 0x35
    #[wasm_bindgen(getter=OpData53)]
    pub fn op_data53() -> u8 {
        0x35
    }
    /// OpData54 = 0x36
    #[wasm_bindgen(getter=OpData54)]
    pub fn op_data54() -> u8 {
        0x36
    }
    /// OpData55 = 0x37
    #[wasm_bindgen(getter=OpData55)]
    pub fn op_data55() -> u8 {
        0x37
    }
    /// OpData56 = 0x38
    #[wasm_bindgen(getter=OpData56)]
    pub fn op_data56() -> u8 {
        0x38
    }
    /// OpData57 = 0x39
    #[wasm_bindgen(getter=OpData57)]
    pub fn op_data57() -> u8 {
        0x39
    }
    /// OpData58 = 0x3a
    #[wasm_bindgen(getter=OpData58)]
    pub fn op_data58() -> u8 {
        0x3a
    }
    /// OpData59 = 0x3b
    #[wasm_bindgen(getter=OpData59)]
    pub fn op_data59() -> u8 {
        0x3b
    }
    /// OpData60 = 0x3c
    #[wasm_bindgen(getter=OpData60)]
    pub fn op_data60() -> u8 {
        0x3c
    }
    /// OpData61 = 0x3d
    #[wasm_bindgen(getter=OpData61)]
    pub fn op_data61() -> u8 {
        0x3d
    }
    /// OpData62 = 0x3e
    #[wasm_bindgen(getter=OpData62)]
    pub fn op_data62() -> u8 {
        0x3e
    }
    /// OpData63 = 0x3f
    #[wasm_bindgen(getter=OpData63)]
    pub fn op_data63() -> u8 {
        0x3f
    }
    /// OpData64 = 0x40
    #[wasm_bindgen(getter=OpData64)]
    pub fn op_data64() -> u8 {
        0x40
    }
    /// OpData65 = 0x41
    #[wasm_bindgen(getter=OpData65)]
    pub fn op_data65() -> u8 {
        0x41
    }
    /// OpData66 = 0x42
    #[wasm_bindgen(getter=OpData66)]
    pub fn op_data66() -> u8 {
        0x42
    }
    /// OpData67 = 0x43
    #[wasm_bindgen(getter=OpData67)]
    pub fn op_data67() -> u8 {
        0x43
    }
    /// OpData68 = 0x44
    #[wasm_bindgen(getter=OpData68)]
    pub fn op_data68() -> u8 {
        0x44
    }
    /// OpData69 = 0x45
    #[wasm_bindgen(getter=OpData69)]
    pub fn op_data69() -> u8 {
        0x45
    }
    /// OpData70 = 0x46
    #[wasm_bindgen(getter=OpData70)]
    pub fn op_data70() -> u8 {
        0x46
    }
    /// OpData71 = 0x47
    #[wasm_bindgen(getter=OpData71)]
    pub fn op_data71() -> u8 {
        0x47
    }
    /// OpData72 = 0x48
    #[wasm_bindgen(getter=OpData72)]
    pub fn op_data72() -> u8 {
        0x48
    }
    /// OpData73 = 0x49
    #[wasm_bindgen(getter=OpData73)]
    pub fn op_data73() -> u8 {
        0x49
    }
    /// OpData74 = 0x4a
    #[wasm_bindgen(getter=OpData74)]
    pub fn op_data74() -> u8 {
        0x4a
    }
    /// OpData75 = 0x4b
    #[wasm_bindgen(getter=OpData75)]
    pub fn op_data75() -> u8 {
        0x4b
    }

    /// OpPushData1 = 0x4c
    #[wasm_bindgen(getter=OpPushData1)]
    pub fn op_pushdata1() -> u8 {
        0x4c
    }
    /// OpPushData2 = 0x4d
    #[wasm_bindgen(getter=OpPushData2)]
    pub fn op_pushdata2() -> u8 {
        0x4d
    }
    /// OpPushData4 = 0x4e
    #[wasm_bindgen(getter=OpPushData4)]
    pub fn op_pushdata4() -> u8 {
        0x4e
    }

    /// Op1Negate = 0x4f
    #[wasm_bindgen(getter=Op1Negate)]
    pub fn op_1negate() -> u8 {
        0x4f
    }

    /// OpReserved = 0x50
    #[wasm_bindgen(getter=OpReserved)]
    pub fn op_reserved() -> u8 {
        0x50
    }

    /// Op1 = 0x51
    #[wasm_bindgen(getter=Op1)]
    pub fn op_1() -> u8 {
        0x51
    }

    /// OpTrue = 0x51,
    #[wasm_bindgen(getter=OpTrue)]
    pub fn op_true() -> u8 {
        0x51
    }

    /// Op2 = 0x52
    #[wasm_bindgen(getter=Op2)]
    pub fn op_2() -> u8 {
        0x52
    }
    /// Op3 = 0x53
    #[wasm_bindgen(getter=Op3)]
    pub fn op_3() -> u8 {
        0x53
    }
    /// Op4 = 0x54
    #[wasm_bindgen(getter=Op4)]
    pub fn op_4() -> u8 {
        0x54
    }
    /// Op5 = 0x55
    #[wasm_bindgen(getter=Op5)]
    pub fn op_5() -> u8 {
        0x55
    }
    /// Op6 = 0x56
    #[wasm_bindgen(getter=Op6)]
    pub fn op_6() -> u8 {
        0x56
    }
    /// Op7 = 0x57
    #[wasm_bindgen(getter=Op7)]
    pub fn op_7() -> u8 {
        0x57
    }
    /// Op8 = 0x58
    #[wasm_bindgen(getter=Op8)]
    pub fn op_8() -> u8 {
        0x58
    }
    /// Op9 = 0x59
    #[wasm_bindgen(getter=Op9)]
    pub fn op_9() -> u8 {
        0x59
    }
    /// Op10 = 0x5a
    #[wasm_bindgen(getter=Op10)]
    pub fn op_10() -> u8 {
        0x5a
    }
    /// Op11 = 0x5b
    #[wasm_bindgen(getter=Op11)]
    pub fn op_11() -> u8 {
        0x5b
    }
    /// Op12 = 0x5c
    #[wasm_bindgen(getter=Op12)]
    pub fn op_12() -> u8 {
        0x5c
    }
    /// Op13 = 0x5d
    #[wasm_bindgen(getter=Op13)]
    pub fn op_13() -> u8 {
        0x5d
    }
    /// Op14 = 0x5e
    #[wasm_bindgen(getter=Op14)]
    pub fn op_14() -> u8 {
        0x5e
    }
    /// Op15 = 0x5f
    #[wasm_bindgen(getter=Op15)]
    pub fn op_15() -> u8 {
        0x5f
    }
    /// Op16 = 0x60
    #[wasm_bindgen(getter=Op16)]
    pub fn op_16() -> u8 {
        0x60
    }

    /// OpNop = 0x61
    #[wasm_bindgen(getter=OpNop)]
    pub fn op_nop() -> u8 {
        0x61
    }
    /// OpVer = 0x62
    #[wasm_bindgen(getter=OpVer)]
    pub fn op_ver() -> u8 {
        0x62
    }
    /// OpIf = 0x63
    #[wasm_bindgen(getter=OpIf)]
    pub fn op_if() -> u8 {
        0x63
    }
    /// OpNotIf = 0x64
    #[wasm_bindgen(getter=OpNotIf)]
    pub fn op_notif() -> u8 {
        0x64
    }
    /// OpVerIf = 0x65
    #[wasm_bindgen(getter=OpVerIf)]
    pub fn op_verif() -> u8 {
        0x65
    }
    /// OpVerNotIf = 0x66
    #[wasm_bindgen(getter=OpVerNotIf)]
    pub fn op_vernotif() -> u8 {
        0x66
    }

    /// OpElse = 0x67
    #[wasm_bindgen(getter=OpElse)]
    pub fn op_else() -> u8 {
        0x67
    }
    /// OpEndIf = 0x68
    #[wasm_bindgen(getter=OpEndIf)]
    pub fn op_endif() -> u8 {
        0x68
    }
    /// OpVerify = 0x69
    #[wasm_bindgen(getter=OpVerify)]
    pub fn op_verify() -> u8 {
        0x69
    }
    /// OpReturn = 0x6a
    #[wasm_bindgen(getter=OpReturn)]
    pub fn op_return() -> u8 {
        0x6a
    }
    /// OpToAltStack = 0x6b
    #[wasm_bindgen(getter=OpToAltStack)]
    pub fn op_toaltstack() -> u8 {
        0x6b
    }
    /// OpFromAltStack = 0x6c
    #[wasm_bindgen(getter=OpFromAltStack)]
    pub fn op_fromaltstack() -> u8 {
        0x6c
    }

    /// Op2Drop = 0x6d
    #[wasm_bindgen(getter=Op2Drop)]
    pub fn op_2drop() -> u8 {
        0x6d
    }
    /// Op2Dup = 0x6e
    #[wasm_bindgen(getter=Op2Dup)]
    pub fn op_2dup() -> u8 {
        0x6e
    }
    /// Op3Dup = 0x6f
    #[wasm_bindgen(getter=Op3Dup)]
    pub fn op_3dup() -> u8 {
        0x6f
    }
    /// Op2Over = 0x70
    #[wasm_bindgen(getter=Op2Over)]
    pub fn op_2over() -> u8 {
        0x70
    }
    /// Op2Rot = 0x71
    #[wasm_bindgen(getter=Op2Rot)]
    pub fn op_2rot() -> u8 {
        0x71
    }
    /// Op2Swap = 0x72
    #[wasm_bindgen(getter=Op2Swap)]
    pub fn op_2swap() -> u8 {
        0x72
    }
    /// OpIfDup = 0x73
    #[wasm_bindgen(getter=OpIfDup)]
    pub fn op_ifdup() -> u8 {
        0x73
    }
    /// OpDepth = 0x74
    #[wasm_bindgen(getter=OpDepth)]
    pub fn op_depth() -> u8 {
        0x74
    }
    /// OpDrop = 0x75
    #[wasm_bindgen(getter=OpDrop)]
    pub fn op_drop() -> u8 {
        0x75
    }
    /// OpDup = 0x76
    #[wasm_bindgen(getter=OpDup)]
    pub fn op_dup() -> u8 {
        0x76
    }
    /// OpNip = 0x77
    #[wasm_bindgen(getter=OpNip)]
    pub fn op_nip() -> u8 {
        0x77
    }
    /// OpOver = 0x78
    #[wasm_bindgen(getter=OpOver)]
    pub fn op_over() -> u8 {
        0x78
    }
    /// OpPick = 0x79
    #[wasm_bindgen(getter=OpPick)]
    pub fn op_pick() -> u8 {
        0x79
    }

    /// OpRoll = 0x7a
    #[wasm_bindgen(getter=OpRoll)]
    pub fn op_roll() -> u8 {
        0x7a
    }
    /// OpRot = 0x7b
    #[wasm_bindgen(getter=OpRot)]
    pub fn op_rot() -> u8 {
        0x7b
    }
    /// OpSwap = 0x7c
    #[wasm_bindgen(getter=OpSwap)]
    pub fn op_swap() -> u8 {
        0x7c
    }
    /// OpTuck = 0x7d
    #[wasm_bindgen(getter=OpTuck)]
    pub fn op_tuck() -> u8 {
        0x7d
    }

    /// Splice opcodes.
    /// OpCat = 0x7e
    #[wasm_bindgen(getter=OpCat)]
    pub fn op_cat() -> u8 {
        0x7e
    }
    /// OpSubStr = 0x7f
    #[wasm_bindgen(getter=OpSubStr)]
    pub fn op_substr() -> u8 {
        0x7f
    }
    /// OpLeft = 0x80
    #[wasm_bindgen(getter=OpLeft)]
    pub fn op_left() -> u8 {
        0x80
    }
    /// OpRight = 0x81
    #[wasm_bindgen(getter=OpRight)]
    pub fn op_right() -> u8 {
        0x81
    }

    /// OpSize = 0x82
    #[wasm_bindgen(getter=OpSize)]
    pub fn op_size() -> u8 {
        0x82
    }

    /// Bitwise logic opcodes.
    /// OpInvert = 0x83
    #[wasm_bindgen(getter=OpInvert)]
    pub fn op_invert() -> u8 {
        0x83
    }
    /// OpAnd = 0x84
    #[wasm_bindgen(getter=OpAnd)]
    pub fn op_and() -> u8 {
        0x84
    }
    /// OpOr = 0x85
    #[wasm_bindgen(getter=OpOr)]
    pub fn op_or() -> u8 {
        0x85
    }
    /// OpXor = 0x86
    #[wasm_bindgen(getter=OpXor)]
    pub fn op_xor() -> u8 {
        0x86
    }

    /// OpEqual = 0x87
    #[wasm_bindgen(getter=OpEqual)]
    pub fn op_equal() -> u8 {
        0x87
    }
    /// OpEqualVerify = 0x88
    #[wasm_bindgen(getter=OpEqualVerify)]
    pub fn op_equalverify() -> u8 {
        0x88
    }

    /// OpReserved1 = 0x89
    #[wasm_bindgen(getter=OpReserved1)]
    pub fn op_reserved1() -> u8 {
        0x89
    }
    /// OpReserved2 = 0x8a
    #[wasm_bindgen(getter=OpReserved2)]
    pub fn op_reserved2() -> u8 {
        0x8a
    }

    /// Numeric related opcodes.
    /// Op1Add = 0x8b
    #[wasm_bindgen(getter=Op1Add)]
    pub fn op_1add() -> u8 {
        0x8b
    }
    /// Op1Sub = 0x8c
    #[wasm_bindgen(getter=Op1Sub)]
    pub fn op_1sub() -> u8 {
        0x8c
    }
    /// Op2Mul = 0x8d
    #[wasm_bindgen(getter=Op2Mul)]
    pub fn op_2mul() -> u8 {
        0x8d
    }
    /// Op2Div = 0x8e
    #[wasm_bindgen(getter=Op2Div)]
    pub fn op_2div() -> u8 {
        0x8e
    }
    /// OpNegate = 0x8f
    #[wasm_bindgen(getter=OpNegate)]
    pub fn op_negate() -> u8 {
        0x8f
    }
    /// OpAbs = 0x90
    #[wasm_bindgen(getter=OpAbs)]
    pub fn op_abs() -> u8 {
        0x90
    }
    /// OpNot = 0x91
    #[wasm_bindgen(getter=OpNot)]
    pub fn op_not() -> u8 {
        0x91
    }
    /// Op0NotEqual = 0x92
    #[wasm_bindgen(getter=Op0NotEqual)]
    pub fn op_0notequal() -> u8 {
        0x92
    }

    /// OpAdd = 0x93
    #[wasm_bindgen(getter=OpAdd)]
    pub fn op_add() -> u8 {
        0x93
    }
    /// OpSub = 0x94
    #[wasm_bindgen(getter=OpSub)]
    pub fn op_sub() -> u8 {
        0x94
    }
    /// OpMul = 0x95
    #[wasm_bindgen(getter=OpMul)]
    pub fn op_mul() -> u8 {
        0x95
    }
    /// OpDiv = 0x96
    #[wasm_bindgen(getter=OpDiv)]
    pub fn op_div() -> u8 {
        0x96
    }
    /// OpMod = 0x97
    #[wasm_bindgen(getter=OpMod)]
    pub fn op_mod() -> u8 {
        0x97
    }
    /// OpLShift = 0x98
    #[wasm_bindgen(getter=OpLShift)]
    pub fn op_lshift() -> u8 {
        0x98
    }
    /// OpRShift = 0x99
    #[wasm_bindgen(getter=OpRShift)]
    pub fn op_rshift() -> u8 {
        0x99
    }

    /// OpBoolAnd = 0x9a
    #[wasm_bindgen(getter=OpBoolAnd)]
    pub fn op_booland() -> u8 {
        0x9a
    }
    /// OpBoolOr = 0x9b
    #[wasm_bindgen(getter=OpBoolOr)]
    pub fn op_boolor() -> u8 {
        0x9b
    }

    /// OpNumEqual = 0x9c
    #[wasm_bindgen(getter=OpNumEqual)]
    pub fn op_numequal() -> u8 {
        0x9c
    }
    /// OpNumEqualVerify = 0x9d
    #[wasm_bindgen(getter=OpNumEqualVerify)]
    pub fn op_numequalverify() -> u8 {
        0x9d
    }
    /// OpNumNotEqual = 0x9e
    #[wasm_bindgen(getter=OpNumNotEqual)]
    pub fn op_numnotequal() -> u8 {
        0x9e
    }

    /// OpLessThan = 0x9f
    #[wasm_bindgen(getter=OpLessThan)]
    pub fn op_lessthan() -> u8 {
        0x9f
    }
    /// OpGreaterThan = 0xa0
    #[wasm_bindgen(getter=OpGreaterThan)]
    pub fn op_greaterthan() -> u8 {
        0xa0
    }
    /// OpLessThanOrEqual = 0xa1
    #[wasm_bindgen(getter=OpLessThanOrEqual)]
    pub fn op_lessthanorequal() -> u8 {
        0xa1
    }
    /// OpGreaterThanOrEqual = 0xa2
    #[wasm_bindgen(getter=OpGreaterThanOrEqual)]
    pub fn op_greaterthanorequal() -> u8 {
        0xa2
    }
    /// OpMin = 0xa3
    #[wasm_bindgen(getter=OpMin)]
    pub fn op_min() -> u8 {
        0xa3
    }
    /// OpMax = 0xa4
    #[wasm_bindgen(getter=OpMax)]
    pub fn op_max() -> u8 {
        0xa4
    }
    /// OpWithin = 0xa5
    #[wasm_bindgen(getter=OpWithin)]
    pub fn op_within() -> u8 {
        0xa5
    }

    /// Undefined opcodes.
    /// OpUnknown166 = 0xa6
    #[wasm_bindgen(getter=OpUnknown166)]
    pub fn op_unknown166() -> u8 {
        0xa6
    }
    /// OpUnknown167 = 0xa7
    #[wasm_bindgen(getter=OpUnknown167)]
    pub fn op_unknown167() -> u8 {
        0xa7
    }

    /// Crypto opcodes.
    /// OpSHA256 = 0xa8
    #[wasm_bindgen(getter=OpSHA256)]
    pub fn op_sha256() -> u8 {
        0xa8
    }

    /// OpCheckMultiSigECDSA = 0xa9
    #[wasm_bindgen(getter=OpCheckMultiSigECDSA)]
    pub fn op_checkmultisigecdsa() -> u8 {
        0xa9
    }

    /// OpBlake2b = 0xaa
    #[wasm_bindgen(getter=OpBlake2b)]
    pub fn op_blake2b() -> u8 {
        0xaa
    }
    /// OpCheckSigECDSA = 0xab
    #[wasm_bindgen(getter=OpCheckSigECDSA)]
    pub fn op_checksigecdsa() -> u8 {
        0xab
    }
    /// OpCheckSig = 0xac
    #[wasm_bindgen(getter=OpCheckSig)]
    pub fn op_checksig() -> u8 {
        0xac
    }
    /// OpCheckSigVerify = 0xad
    #[wasm_bindgen(getter=OpCheckSigVerify)]
    pub fn op_checksigverify() -> u8 {
        0xad
    }
    /// OpCheckMultiSig = 0xae
    #[wasm_bindgen(getter=OpCheckMultiSig)]
    pub fn op_checkmultisig() -> u8 {
        0xae
    }
    /// OpCheckMultiSigVerify = 0xaf
    #[wasm_bindgen(getter=OpCheckMultiSigVerify)]
    pub fn op_checkmultisigverify() -> u8 {
        0xaf
    }
    /// OpCheckLockTimeVerify = 0xb0
    #[wasm_bindgen(getter=OpCheckLockTimeVerify)]
    pub fn op_checklocktimeverify() -> u8 {
        0xb0
    }
    /// OpCheckSequenceVerify = 0xb1
    #[wasm_bindgen(getter=OpCheckSequenceVerify)]
    pub fn op_checksequenceverify() -> u8 {
        0xb1
    }

    /// Undefined opcodes.
    /// OpUnknown178 = 0xb2
    #[wasm_bindgen(getter=OpUnknown178)]
    pub fn op_unknown178() -> u8 {
        0xb2
    }
    /// OpUnknown179 = 0xb3
    #[wasm_bindgen(getter=OpUnknown179)]
    pub fn op_unknown179() -> u8 {
        0xb3
    }
    /// OpUnknown180 = 0xb4
    #[wasm_bindgen(getter=OpUnknown180)]
    pub fn op_unknown180() -> u8 {
        0xb4
    }
    /// OpUnknown181 = 0xb5
    #[wasm_bindgen(getter=OpUnknown181)]
    pub fn op_unknown181() -> u8 {
        0xb5
    }
    /// OpUnknown182 = 0xb6
    #[wasm_bindgen(getter=OpUnknown182)]
    pub fn op_unknown182() -> u8 {
        0xb6
    }
    /// OpUnknown183 = 0xb7
    #[wasm_bindgen(getter=OpUnknown183)]
    pub fn op_unknown183() -> u8 {
        0xb7
    }
    /// OpUnknown184 = 0xb8
    #[wasm_bindgen(getter=OpUnknown184)]
    pub fn op_unknown184() -> u8 {
        0xb8
    }
    /// OpUnknown185 = 0xb9
    #[wasm_bindgen(getter=OpUnknown185)]
    pub fn op_unknown185() -> u8 {
        0xb9
    }
    /// OpUnknown186 = 0xba
    #[wasm_bindgen(getter=OpUnknown186)]
    pub fn op_unknown186() -> u8 {
        0xba
    }
    /// OpUnknown187 = 0xbb
    #[wasm_bindgen(getter=OpUnknown187)]
    pub fn op_unknown187() -> u8 {
        0xbb
    }
    /// OpUnknown188 = 0xbc
    #[wasm_bindgen(getter=OpUnknown188)]
    pub fn op_unknown188() -> u8 {
        0xbc
    }
    /// OpUnknown189 = 0xbd
    #[wasm_bindgen(getter=OpUnknown189)]
    pub fn op_unknown189() -> u8 {
        0xbd
    }
    /// OpUnknown190 = 0xbe
    #[wasm_bindgen(getter=OpUnknown190)]
    pub fn op_unknown190() -> u8 {
        0xbe
    }
    /// OpUnknown191 = 0xbf
    #[wasm_bindgen(getter=OpUnknown191)]
    pub fn op_unknown191() -> u8 {
        0xbf
    }
    /// OpUnknown192 = 0xc0
    #[wasm_bindgen(getter=OpUnknown192)]
    pub fn op_unknown192() -> u8 {
        0xc0
    }
    /// OpUnknown193 = 0xc1
    #[wasm_bindgen(getter=OpUnknown193)]
    pub fn op_unknown193() -> u8 {
        0xc1
    }
    /// OpUnknown194 = 0xc2
    #[wasm_bindgen(getter=OpUnknown194)]
    pub fn op_unknown194() -> u8 {
        0xc2
    }
    /// OpUnknown195 = 0xc3
    #[wasm_bindgen(getter=OpUnknown195)]
    pub fn op_unknown195() -> u8 {
        0xc3
    }
    /// OpUnknown196 = 0xc4
    #[wasm_bindgen(getter=OpUnknown196)]
    pub fn op_unknown196() -> u8 {
        0xc4
    }
    /// OpUnknown197 = 0xc5
    #[wasm_bindgen(getter=OpUnknown197)]
    pub fn op_unknown197() -> u8 {
        0xc5
    }
    /// OpUnknown198 = 0xc6
    #[wasm_bindgen(getter=OpUnknown198)]
    pub fn op_unknown198() -> u8 {
        0xc6
    }
    /// OpUnknown199 = 0xc7
    #[wasm_bindgen(getter=OpUnknown199)]
    pub fn op_unknown199() -> u8 {
        0xc7
    }
    /// OpUnknown200 = 0xc8
    #[wasm_bindgen(getter=OpUnknown200)]
    pub fn op_unknown200() -> u8 {
        0xc8
    }
    /// OpUnknown201 = 0xc9
    #[wasm_bindgen(getter=OpUnknown201)]
    pub fn op_unknown201() -> u8 {
        0xc9
    }
    /// OpUnknown202 = 0xca
    #[wasm_bindgen(getter=OpUnknown202)]
    pub fn op_unknown202() -> u8 {
        0xca
    }
    /// OpUnknown203 = 0xcb
    #[wasm_bindgen(getter=OpUnknown203)]
    pub fn op_unknown203() -> u8 {
        0xcb
    }
    /// OpUnknown204 = 0xcc
    #[wasm_bindgen(getter=OpUnknown204)]
    pub fn op_unknown204() -> u8 {
        0xcc
    }
    /// OpUnknown205 = 0xcd
    #[wasm_bindgen(getter=OpUnknown205)]
    pub fn op_unknown205() -> u8 {
        0xcd
    }
    /// OpUnknown206 = 0xce
    #[wasm_bindgen(getter=OpUnknown206)]
    pub fn op_unknown206() -> u8 {
        0xce
    }
    /// OpUnknown207 = 0xcf
    #[wasm_bindgen(getter=OpUnknown207)]
    pub fn op_unknown207() -> u8 {
        0xcf
    }
    /// OpUnknown208 = 0xd0
    #[wasm_bindgen(getter=OpUnknown208)]
    pub fn op_unknown208() -> u8 {
        0xd0
    }
    /// OpUnknown209 = 0xd1
    #[wasm_bindgen(getter=OpUnknown209)]
    pub fn op_unknown209() -> u8 {
        0xd1
    }
    /// OpUnknown210 = 0xd2
    #[wasm_bindgen(getter=OpUnknown210)]
    pub fn op_unknown210() -> u8 {
        0xd2
    }
    /// OpUnknown211 = 0xd3
    #[wasm_bindgen(getter=OpUnknown211)]
    pub fn op_unknown211() -> u8 {
        0xd3
    }
    /// OpUnknown212 = 0xd4
    #[wasm_bindgen(getter=OpUnknown212)]
    pub fn op_unknown212() -> u8 {
        0xd4
    }
    /// OpUnknown213 = 0xd5
    #[wasm_bindgen(getter=OpUnknown213)]
    pub fn op_unknown213() -> u8 {
        0xd5
    }
    /// OpUnknown214 = 0xd6
    #[wasm_bindgen(getter=OpUnknown214)]
    pub fn op_unknown214() -> u8 {
        0xd6
    }
    /// OpUnknown215 = 0xd7
    #[wasm_bindgen(getter=OpUnknown215)]
    pub fn op_unknown215() -> u8 {
        0xd7
    }
    /// OpUnknown216 = 0xd8
    #[wasm_bindgen(getter=OpUnknown216)]
    pub fn op_unknown216() -> u8 {
        0xd8
    }
    /// OpUnknown217 = 0xd9
    #[wasm_bindgen(getter=OpUnknown217)]
    pub fn op_unknown217() -> u8 {
        0xd9
    }
    /// OpUnknown218 = 0xda
    #[wasm_bindgen(getter=OpUnknown218)]
    pub fn op_unknown218() -> u8 {
        0xda
    }
    /// OpUnknown219 = 0xdb
    #[wasm_bindgen(getter=OpUnknown219)]
    pub fn op_unknown219() -> u8 {
        0xdb
    }
    /// OpUnknown220 = 0xdc
    #[wasm_bindgen(getter=OpUnknown220)]
    pub fn op_unknown220() -> u8 {
        0xdc
    }
    /// OpUnknown221 = 0xdd
    #[wasm_bindgen(getter=OpUnknown221)]
    pub fn op_unknown221() -> u8 {
        0xdd
    }
    /// OpUnknown222 = 0xde
    #[wasm_bindgen(getter=OpUnknown222)]
    pub fn op_unknown222() -> u8 {
        0xde
    }
    /// OpUnknown223 = 0xdf
    #[wasm_bindgen(getter=OpUnknown223)]
    pub fn op_unknown223() -> u8 {
        0xdf
    }
    /// OpUnknown224 = 0xe0
    #[wasm_bindgen(getter=OpUnknown224)]
    pub fn op_unknown224() -> u8 {
        0xe0
    }
    /// OpUnknown225 = 0xe1
    #[wasm_bindgen(getter=OpUnknown225)]
    pub fn op_unknown225() -> u8 {
        0xe1
    }
    /// OpUnknown226 = 0xe2
    #[wasm_bindgen(getter=OpUnknown226)]
    pub fn op_unknown226() -> u8 {
        0xe2
    }
    /// OpUnknown227 = 0xe3
    #[wasm_bindgen(getter=OpUnknown227)]
    pub fn op_unknown227() -> u8 {
        0xe3
    }
    /// OpUnknown228 = 0xe4
    #[wasm_bindgen(getter=OpUnknown228)]
    pub fn op_unknown228() -> u8 {
        0xe4
    }
    /// OpUnknown229 = 0xe5
    #[wasm_bindgen(getter=OpUnknown229)]
    pub fn op_unknown229() -> u8 {
        0xe5
    }
    /// OpUnknown230 = 0xe6
    #[wasm_bindgen(getter=OpUnknown230)]
    pub fn op_unknown230() -> u8 {
        0xe6
    }
    /// OpUnknown231 = 0xe7
    #[wasm_bindgen(getter=OpUnknown231)]
    pub fn op_unknown231() -> u8 {
        0xe7
    }
    /// OpUnknown232 = 0xe8
    #[wasm_bindgen(getter=OpUnknown232)]
    pub fn op_unknown232() -> u8 {
        0xe8
    }
    /// OpUnknown233 = 0xe9
    #[wasm_bindgen(getter=OpUnknown233)]
    pub fn op_unknown233() -> u8 {
        0xe9
    }
    /// OpUnknown234 = 0xea
    #[wasm_bindgen(getter=OpUnknown234)]
    pub fn op_unknown234() -> u8 {
        0xea
    }
    /// OpUnknown235 = 0xeb
    #[wasm_bindgen(getter=OpUnknown235)]
    pub fn op_unknown235() -> u8 {
        0xeb
    }
    /// OpUnknown236 = 0xec
    #[wasm_bindgen(getter=OpUnknown236)]
    pub fn op_unknown236() -> u8 {
        0xec
    }
    /// OpUnknown237 = 0xed
    #[wasm_bindgen(getter=OpUnknown237)]
    pub fn op_unknown237() -> u8 {
        0xed
    }
    /// OpUnknown238 = 0xee
    #[wasm_bindgen(getter=OpUnknown238)]
    pub fn op_unknown238() -> u8 {
        0xee
    }
    /// OpUnknown239 = 0xef
    #[wasm_bindgen(getter=OpUnknown239)]
    pub fn op_unknown239() -> u8 {
        0xef
    }
    /// OpUnknown240 = 0xf0
    #[wasm_bindgen(getter=OpUnknown240)]
    pub fn op_unknown240() -> u8 {
        0xf0
    }
    /// OpUnknown241 = 0xf1
    #[wasm_bindgen(getter=OpUnknown241)]
    pub fn op_unknown241() -> u8 {
        0xf1
    }
    /// OpUnknown242 = 0xf2
    #[wasm_bindgen(getter=OpUnknown242)]
    pub fn op_unknown242() -> u8 {
        0xf2
    }
    /// OpUnknown243 = 0xf3
    #[wasm_bindgen(getter=OpUnknown243)]
    pub fn op_unknown243() -> u8 {
        0xf3
    }
    /// OpUnknown244 = 0xf4
    #[wasm_bindgen(getter=OpUnknown244)]
    pub fn op_unknown244() -> u8 {
        0xf4
    }
    /// OpUnknown245 = 0xf5
    #[wasm_bindgen(getter=OpUnknown245)]
    pub fn op_unknown245() -> u8 {
        0xf5
    }
    /// OpUnknown246 = 0xf6
    #[wasm_bindgen(getter=OpUnknown246)]
    pub fn op_unknown246() -> u8 {
        0xf6
    }
    /// OpUnknown247 = 0xf7
    #[wasm_bindgen(getter=OpUnknown247)]
    pub fn op_unknown247() -> u8 {
        0xf7
    }
    /// OpUnknown248 = 0xf8
    #[wasm_bindgen(getter=OpUnknown248)]
    pub fn op_unknown248() -> u8 {
        0xf8
    }
    /// OpUnknown249 = 0xf9
    #[wasm_bindgen(getter=OpUnknown249)]
    pub fn op_unknown249() -> u8 {
        0xf9
    }

    ///

    /// OpSmallInteger = 0xfa
    #[wasm_bindgen(getter=OpSmallInteger)]
    pub fn op_smallinteger() -> u8 {
        0xfa
    }
    /// OpPubKeys = 0xfb
    #[wasm_bindgen(getter=OpPubKeys)]
    pub fn op_pubkeys() -> u8 {
        0xfb
    }
    /// OpUnknown252 = 0xfc
    #[wasm_bindgen(getter=OpUnknown252)]
    pub fn op_unknown252() -> u8 {
        0xfc
    }
    /// OpPubKeyHash = 0xfd
    #[wasm_bindgen(getter=OpPubKeyHash)]
    pub fn op_pubkeyhash() -> u8 {
        0xfd
    }
    /// OpPubKey = 0xfe
    #[wasm_bindgen(getter=OpPubKey)]
    pub fn op_pubkey() -> u8 {
        0xfe
    }
    /// OpInvalidOpCode = 0xff
    #[wasm_bindgen(getter=OpInvalidOpCode)]
    pub fn op_invalidopcode() -> u8 {
        0xff
    }
}

///// Kaspa Transaction Script Opcodes
///// @see {@link ScriptBuilder}
///// @category Consensus
/////
// #[wasm_bindgen]
// pub enum Opcode{

//     Op0 = 0x00,
//     // OpFalse = 0x00,

//     OpData1 = 0x01,
//     OpData2 = 0x02,
//     OpData3 = 0x03,
//     OpData4 = 0x04,
//     OpData5 = 0x05,
//     OpData6 = 0x06,
//     OpData7 = 0x07,
//     OpData8 = 0x08,
//     OpData9 = 0x09,
//     OpData10 = 0x0a,
//     OpData11 = 0x0b,
//     OpData12 = 0x0c,
//     OpData13 = 0x0d,
//     OpData14 = 0x0e,
//     OpData15 = 0x0f,
//     OpData16 = 0x10,
//     OpData17 = 0x11,
//     OpData18 = 0x12,
//     OpData19 = 0x13,
//     OpData20 = 0x14,
//     OpData21 = 0x15,
//     OpData22 = 0x16,
//     OpData23 = 0x17,
//     OpData24 = 0x18,
//     OpData25 = 0x19,
//     OpData26 = 0x1a,
//     OpData27 = 0x1b,
//     OpData28 = 0x1c,
//     OpData29 = 0x1d,
//     OpData30 = 0x1e,
//     OpData31 = 0x1f,
//     OpData32 = 0x20,
//     OpData33 = 0x21,
//     OpData34 = 0x22,
//     OpData35 = 0x23,
//     OpData36 = 0x24,
//     OpData37 = 0x25,
//     OpData38 = 0x26,
//     OpData39 = 0x27,
//     OpData40 = 0x28,
//     OpData41 = 0x29,
//     OpData42 = 0x2a,
//     OpData43 = 0x2b,
//     OpData44 = 0x2c,
//     OpData45 = 0x2d,
//     OpData46 = 0x2e,
//     OpData47 = 0x2f,
//     OpData48 = 0x30,
//     OpData49 = 0x31,
//     OpData50 = 0x32,
//     OpData51 = 0x33,
//     OpData52 = 0x34,
//     OpData53 = 0x35,
//     OpData54 = 0x36,
//     OpData55 = 0x37,
//     OpData56 = 0x38,
//     OpData57 = 0x39,
//     OpData58 = 0x3a,
//     OpData59 = 0x3b,
//     OpData60 = 0x3c,
//     OpData61 = 0x3d,
//     OpData62 = 0x3e,
//     OpData63 = 0x3f,
//     OpData64 = 0x40,
//     OpData65 = 0x41,
//     OpData66 = 0x42,
//     OpData67 = 0x43,
//     OpData68 = 0x44,
//     OpData69 = 0x45,
//     OpData70 = 0x46,
//     OpData71 = 0x47,
//     OpData72 = 0x48,
//     OpData73 = 0x49,
//     OpData74 = 0x4a,
//     OpData75 = 0x4b,

//     OpPushData1 = 0x4c,
//     OpPushData2 = 0x4d,
//     OpPushData4 = 0x4e,

//     Op1Negate = 0x4f,

//     OpReserved = 0x50,

//     Op1 = 0x51,
//     // OpTrue = 0x51,
//     Op2 = 0x52,
//     Op3 = 0x53,
//     Op4 = 0x54,
//     Op5 = 0x55,
//     Op6 = 0x56,
//     Op7 = 0x57,
//     Op8 = 0x58,
//     Op9 = 0x59,
//     Op10 = 0x5a,
//     Op11 = 0x5b,
//     Op12 = 0x5c,
//     Op13 = 0x5d,
//     Op14 = 0x5e,
//     Op15 = 0x5f,
//     Op16 = 0x60,

//     OpNop = 0x61,
//     OpVer = 0x62,
//     OpIf = 0x63,
//     OpNotIf = 0x64,
//     OpVerIf = 0x65,
//     OpVerNotIf = 0x66,

//     OpElse = 0x67,
//     OpEndIf = 0x68,
//     OpVerify = 0x69,
//     OpReturn = 0x6a,
//     OpToAltStack = 0x6b,
//     OpFromAltStack = 0x6c,

//     Op2Drop = 0x6d,
//     Op2Dup = 0x6e,
//     Op3Dup = 0x6f,
//     Op2Over = 0x70,
//     Op2Rot = 0x71,
//     Op2Swap = 0x72,
//     OpIfDup = 0x73,
//     OpDepth = 0x74,
//     OpDrop = 0x75,
//     OpDup = 0x76,
//     OpNip = 0x77,
//     OpOver = 0x78,
//     OpPick = 0x79,

//     OpRoll = 0x7a,
//     OpRot = 0x7b,
//     OpSwap = 0x7c,
//     OpTuck = 0x7d,

//    /// Splice opcodes.
//     OpCat = 0x7e,
//     OpSubStr = 0x7f,
//     OpLeft = 0x80,
//     OpRight = 0x81,

//     OpSize = 0x82,

//    /// Bitwise logic opcodes.
//     OpInvert = 0x83,
//     OpAnd = 0x84,
//     OpOr = 0x85,
//     OpXor = 0x86,

//     OpEqual = 0x87,
//     OpEqualVerify = 0x88,

//     OpReserved1 = 0x89,
//     OpReserved2 = 0x8a,

//    /// Numeric related opcodes.
//     Op1Add = 0x8b,
//     Op1Sub = 0x8c,
//     Op2Mul = 0x8d,
//     Op2Div = 0x8e,
//     OpNegate = 0x8f,
//     OpAbs = 0x90,
//     OpNot = 0x91,
//     Op0NotEqual = 0x92,

//     OpAdd = 0x93,
//     OpSub = 0x94,
//     OpMul = 0x95,
//     OpDiv = 0x96,
//     OpMod = 0x97,
//     OpLShift = 0x98,
//     OpRShift = 0x99,

//     OpBoolAnd = 0x9a,
//     OpBoolOr = 0x9b,

//     OpNumEqual = 0x9c,
//     OpNumEqualVerify = 0x9d,
//     OpNumNotEqual = 0x9e,

//     OpLessThan = 0x9f,
//     OpGreaterThan = 0xa0,
//     OpLessThanOrEqual = 0xa1,
//     OpGreaterThanOrEqual = 0xa2,
//     OpMin = 0xa3,
//     OpMax = 0xa4,
//     OpWithin = 0xa5,

//    /// Undefined opcodes.
//     OpUnknown166 = 0xa6,
//     OpUnknown167 = 0xa7,

//    /// Crypto opcodes.
//     OpSHA256 = 0xa8,

//     OpCheckMultiSigECDSA = 0xa9,

//     OpBlake2b = 0xaa,
//     OpCheckSigECDSA = 0xab,
//     OpCheckSig = 0xac,
//     OpCheckSigVerify = 0xad,
//     OpCheckMultiSig = 0xae,
//     OpCheckMultiSigVerify = 0xaf,
//     OpCheckLockTimeVerify = 0xb0,
//     OpCheckSequenceVerify = 0xb1,

//    /// Undefined opcodes.
//     OpUnknown178 = 0xb2,
//     OpUnknown179 = 0xb3,
//     OpUnknown180 = 0xb4,
//     OpUnknown181 = 0xb5,
//     OpUnknown182 = 0xb6,
//     OpUnknown183 = 0xb7,
//     OpUnknown184 = 0xb8,
//     OpUnknown185 = 0xb9,
//     OpUnknown186 = 0xba,
//     OpUnknown187 = 0xbb,
//     OpUnknown188 = 0xbc,
//     OpUnknown189 = 0xbd,
//     OpUnknown190 = 0xbe,
//     OpUnknown191 = 0xbf,
//     OpUnknown192 = 0xc0,
//     OpUnknown193 = 0xc1,
//     OpUnknown194 = 0xc2,
//     OpUnknown195 = 0xc3,
//     OpUnknown196 = 0xc4,
//     OpUnknown197 = 0xc5,
//     OpUnknown198 = 0xc6,
//     OpUnknown199 = 0xc7,
//     OpUnknown200 = 0xc8,
//     OpUnknown201 = 0xc9,
//     OpUnknown202 = 0xca,
//     OpUnknown203 = 0xcb,
//     OpUnknown204 = 0xcc,
//     OpUnknown205 = 0xcd,
//     OpUnknown206 = 0xce,
//     OpUnknown207 = 0xcf,
//     OpUnknown208 = 0xd0,
//     OpUnknown209 = 0xd1,
//     OpUnknown210 = 0xd2,
//     OpUnknown211 = 0xd3,
//     OpUnknown212 = 0xd4,
//     OpUnknown213 = 0xd5,
//     OpUnknown214 = 0xd6,
//     OpUnknown215 = 0xd7,
//     OpUnknown216 = 0xd8,
//     OpUnknown217 = 0xd9,
//     OpUnknown218 = 0xda,
//     OpUnknown219 = 0xdb,
//     OpUnknown220 = 0xdc,
//     OpUnknown221 = 0xdd,
//     OpUnknown222 = 0xde,
//     OpUnknown223 = 0xdf,
//     OpUnknown224 = 0xe0,
//     OpUnknown225 = 0xe1,
//     OpUnknown226 = 0xe2,
//     OpUnknown227 = 0xe3,
//     OpUnknown228 = 0xe4,
//     OpUnknown229 = 0xe5,
//     OpUnknown230 = 0xe6,
//     OpUnknown231 = 0xe7,
//     OpUnknown232 = 0xe8,
//     OpUnknown233 = 0xe9,
//     OpUnknown234 = 0xea,
//     OpUnknown235 = 0xeb,
//     OpUnknown236 = 0xec,
//     OpUnknown237 = 0xed,
//     OpUnknown238 = 0xee,
//     OpUnknown239 = 0xef,
//     OpUnknown240 = 0xf0,
//     OpUnknown241 = 0xf1,
//     OpUnknown242 = 0xf2,
//     OpUnknown243 = 0xf3,
//     OpUnknown244 = 0xf4,
//     OpUnknown245 = 0xf5,
//     OpUnknown246 = 0xf6,
//     OpUnknown247 = 0xf7,
//     OpUnknown248 = 0xf8,
//     OpUnknown249 = 0xf9,

//    ///
//     OpSmallInteger = 0xfa,
//     OpPubKeys = 0xfb,
//     OpUnknown252 = 0xfc,
//     OpPubKeyHash = 0xfd,
//     OpPubKey = 0xfe,
//     OpInvalidOpCode = 0xff,

// }
