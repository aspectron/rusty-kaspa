const EMPTY_LIST: [u8; 0] = [];

pub fn serialize_to_vector<Data>(v: &mut Vec<u8>, data: &Data) -> bool
where
    Data: ToBytes,
{
    let bytes = data.to_bytes();
    let size = bytes.len() as u64;
    // if size > 0 {
    v.extend_from_slice(&size.compact_size());
    v.extend_from_slice(bytes);
    true
    // }else{
    //     false
    // }
}

pub trait ToBytes {
    fn to_bytes(&self) -> &[u8];
}

impl ToBytes for Vec<u8> {
    fn to_bytes(&self) -> &[u8] {
        self
    }
}

impl ToBytes for Option<String> {
    fn to_bytes(&self) -> &[u8] {
        if let Some(str) = self {
            str.as_bytes()
        } else {
            &EMPTY_LIST
        }
    }
}

pub trait CompactSize {
    fn compact_size(self) -> Vec<u8>;
}

impl CompactSize for u8 {
    fn compact_size(self) -> Vec<u8> {
        if self < 253 {
            vec![self]
        } else {
            vec![253, self]
        }
    }
}

impl CompactSize for u16 {
    fn compact_size(self) -> Vec<u8> {
        if self < 253 {
            vec![self as u8]
        } else {
            let mut v = vec![253];
            v.extend_from_slice(&self.to_le_bytes());
            v
        }
    }
}

impl CompactSize for u32 {
    fn compact_size(self) -> Vec<u8> {
        if self < 253 {
            vec![self as u8]
        } else if self <= u16::MAX as u32 {
            let mut v = vec![253];
            v.extend_from_slice(&self.to_le_bytes());
            v
        } else {
            let mut v = vec![254];
            v.extend_from_slice(&self.to_le_bytes());
            v
        }
    }
}

impl CompactSize for u64 {
    fn compact_size(self) -> Vec<u8> {
        if self < 253 {
            vec![self as u8]
        } else if self <= u16::MAX as u64 {
            let mut v = vec![253];
            v.extend_from_slice(&self.to_le_bytes());
            v
        } else if self <= u32::MAX as u64 {
            let mut v = vec![254];
            v.extend_from_slice(&self.to_le_bytes());
            v
        } else {
            let mut v = vec![255];
            v.extend_from_slice(&self.to_le_bytes());
            v
        }
    }
}
