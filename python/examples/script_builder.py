import json 

from kaspa import Opcodes, ScriptBuilder, PrivateKey


if __name__ == "__main__":
    private_key = PrivateKey("389840d7696e89c38856a066175e8e92697f0cf182b854c883237a50acaf1f69")
    keypair = private_key.to_keypair()

    data = {"p": "krc-20", "op": "mint", "tick": "TNACHO"}

    script = ScriptBuilder()
    script.add_data(keypair.public_key)
    script.add_op(Opcodes.OpCheckSig)
    script.add_op(Opcodes.OpFalse)
    script.add_op(Opcodes.OpIf)
    script.add_data(b"kasplex")
    script.add_i64(0)
    script.add_data(json.dumps(data, separators=(',', ':')).encode('utf-8'))
    script.add_op(Opcodes.OpEndIf)

    print(script.to_string())
    p2sh_address = script.create_pay_to_script_hash_script()
