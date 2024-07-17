from typing import Tuple


class Address:
    
    def __init__(self, address:str) -> None: ...
    
    def to_string(self) -> str: ...

    def version(self) -> str: ...

    def set_prefix(self, prefix:str) -> None: ...

    def payload(self) -> str: ...

    @staticmethod
    def validate(address: str) -> bool: ...


class PrivateKeyGenerator:
    
    def __init__(self, xprv:str, is_multisig:bool, account_index:int, cosigner_index:int) -> Tuple[str, str]: ...
    
    def receive_key(self, index:int) -> PrivateKey: ...

    def change_key(self, index:int) -> PrivateKey: ...



class PrivateKey:
    
    def __init__(self, secret_key:str) -> None: ...

    def to_string(self) -> str: ...

    def to_public_key(self) -> PublicKey: ...

    def to_address(self, network:str) -> Address: ...

    def to_address_ecdsa(self, network:str) -> Address: ...
    
    @staticmethod
    def try_new(key:str) -> PrivateKey: ...



class PublicKey:
    def __init__(self, key:str) -> None: ...

    def to_string(self) -> str: ...

    def to_address(self, network:str) -> Address: ...

    def to_address_ecdsa(self, network:str) -> Address: ...



class RpcClient:
    
    def __init__(self, url:str) -> None: ...

    def is_connected(self) -> bool: ...
    
    def connect(self) -> None: ...

    def disconnect(self) -> None: ...
