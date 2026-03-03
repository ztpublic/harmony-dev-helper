use crate::error::HdcError;

pub const MAX_CONNECT_KEY_SIZE: usize = 32;
pub const BANNER_SIZE: usize = 12;

#[derive(Debug, Clone)]
pub struct ChannelHandshake {
    pub banner: [u8; BANNER_SIZE],
    #[allow(dead_code)]
    pub channel_id: u32,
    pub connect_key: String,
}

impl ChannelHandshake {
    pub fn deserialize(buf: &[u8]) -> Result<Self, HdcError> {
        if buf.len() < BANNER_SIZE + 4 {
            return Err(HdcError::Protocol(format!(
                "invalid handshake payload length: {}",
                buf.len()
            )));
        }

        let mut banner = [0u8; BANNER_SIZE];
        banner.copy_from_slice(&buf[0..BANNER_SIZE]);

        let mut id_bytes = [0u8; 4];
        id_bytes.copy_from_slice(&buf[BANNER_SIZE..BANNER_SIZE + 4]);

        Ok(Self {
            banner,
            channel_id: u32::from_be_bytes(id_bytes),
            connect_key: String::new(),
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(BANNER_SIZE + MAX_CONNECT_KEY_SIZE);
        out.extend_from_slice(&self.banner);

        let mut connect_key = [0u8; MAX_CONNECT_KEY_SIZE];
        let raw = self.connect_key.as_bytes();
        let copy_len = raw.len().min(MAX_CONNECT_KEY_SIZE);
        connect_key[0..copy_len].copy_from_slice(&raw[0..copy_len]);
        out.extend_from_slice(&connect_key);

        out
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelHandshake, BANNER_SIZE, MAX_CONNECT_KEY_SIZE};

    #[test]
    fn deserialize_and_serialize_roundtrip_shape() {
        let mut payload = vec![0u8; BANNER_SIZE + 4];
        payload[0..BANNER_SIZE].copy_from_slice(b"OHOS HDC.HEL");
        payload[BANNER_SIZE..BANNER_SIZE + 4].copy_from_slice(&42u32.to_be_bytes());

        let mut handshake = ChannelHandshake::deserialize(&payload).unwrap();
        handshake.connect_key = "abc".to_string();

        let serialized = handshake.serialize();
        assert_eq!(serialized.len(), BANNER_SIZE + MAX_CONNECT_KEY_SIZE);
        assert_eq!(&serialized[0..BANNER_SIZE], b"OHOS HDC.HEL");
        assert_eq!(&serialized[BANNER_SIZE..BANNER_SIZE + 3], b"abc");
    }
}
