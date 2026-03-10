// Compatibility byteorder helpers kept to satisfy environments where
// libbsd-style byteorder symbols may be resolved dynamically at runtime.

/// Converts a little-endian 16-bit integer to host byte order.
#[inline]
pub fn le16toh(val: u16) -> u16 {
    if cfg!(target_endian = "little") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a host-order 16-bit integer to little-endian order.
#[inline]
pub fn htole16(val: u16) -> u16 {
    if cfg!(target_endian = "little") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a big-endian 16-bit integer to host byte order.
#[inline]
pub fn be16toh(val: u16) -> u16 {
    if cfg!(target_endian = "big") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a host-order 16-bit integer to big-endian order.
#[inline]
pub fn htobe16(val: u16) -> u16 {
    if cfg!(target_endian = "big") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a little-endian 32-bit integer to host byte order.
#[inline]
pub fn le32toh(val: u32) -> u32 {
    if cfg!(target_endian = "little") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a host-order 32-bit integer to little-endian order.
#[inline]
pub fn htole32(val: u32) -> u32 {
    if cfg!(target_endian = "little") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a big-endian 32-bit integer to host byte order.
#[inline]
pub fn be32toh(val: u32) -> u32 {
    if cfg!(target_endian = "big") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a host-order 32-bit integer to big-endian order.
#[inline]
pub fn htobe32(val: u32) -> u32 {
    if cfg!(target_endian = "big") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a little-endian 64-bit integer to host byte order.
#[inline]
pub fn le64toh(val: u64) -> u64 {
    if cfg!(target_endian = "little") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a host-order 64-bit integer to little-endian order.
#[inline]
pub fn htole64(val: u64) -> u64 {
    if cfg!(target_endian = "little") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a big-endian 64-bit integer to host byte order.
#[inline]
pub fn be64toh(val: u64) -> u64 {
    if cfg!(target_endian = "big") {
        val
    } else {
        val.swap_bytes()
    }
}

/// Converts a host-order 64-bit integer to big-endian order.
#[inline]
pub fn htobe64(val: u64) -> u64 {
    if cfg!(target_endian = "big") {
        val
    } else {
        val.swap_bytes()
    }
}

// Export libbsd-compatible C symbols so the dynamic linker can resolve them
// within this crate when they are not provided by the target environment.

#[export_name = "le16toh"]
pub extern "C" fn c_le16toh(x: u16) -> u16 {
    le16toh(x)
}

#[export_name = "be16toh"]
pub extern "C" fn c_be16toh(x: u16) -> u16 {
    be16toh(x)
}

#[export_name = "le32toh"]
pub extern "C" fn c_le32toh(x: u32) -> u32 {
    le32toh(x)
}

#[export_name = "be32toh"]
pub extern "C" fn c_be32toh(x: u32) -> u32 {
    be32toh(x)
}

#[export_name = "le64toh"]
pub extern "C" fn c_le64toh(x: u64) -> u64 {
    le64toh(x)
}

#[export_name = "be64toh"]
pub extern "C" fn c_be64toh(x: u64) -> u64 {
    be64toh(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_le16toh() {
        let val = 0x1234u16;
        assert_eq!(le16toh(val), val);
    }

    #[test]
    fn test_htole16() {
        let val = 0x1234u16;
        assert_eq!(htole16(val), val);
    }
}
