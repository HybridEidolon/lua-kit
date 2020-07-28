//! Reading Lua Binary Chunks from IO sources.

use crate::types::{
    Chunk,
    ChunkHeader,
    LuaEndianness,
    LuaInteger,
    LuaNumber,
    ValueSize,
    Version,
};

use std::io::{self, Read};

use byteorder::{BE, LE, ByteOrder, ReadBytesExt};


// common binary chunk signature
const LUA_SIGNATURE: &'static [u8] = b"\x1bLua";
// used by lua 5.3
const DATA: &'static [u8] = b"\x19\x93\r\n\x1a\n";
// A test integer to know endianness.
const TEST_INT: i64 = 0x5678;
// A test floating-point number to know endianness.
const TEST_NUMBER: f64 = 370.5;

fn field_error(e: io::Error, name: &str) -> io::Error {
    io::Error::new(
        e.kind(),
        format!("Unable to read field \"{}\": {}", name, e),
    )
}

pub fn read_chunk<R: Read>(mut r: R) -> io::Result<Chunk> {
    let header = read_header(&mut r)?;
    todo!()
}

fn read_header<R: Read>(mut r: R) -> io::Result<ChunkHeader> {
    let mut buf = [0u8; 8];

    // Check for LuaJit first, even though unsupported atm.
    r.read_exact(&mut buf[0..3]).map_err(|e| field_error(e, "signature"))?;
    if &buf[..3] == &b"\x01LJ"[..] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LuaJit chunk detected, but this library does not support LuaJit",
        ));
    }

    // Regular Lua 5x signature
    r.read_exact(&mut buf[3..4]).map_err(|e| field_error(e, "signature"))?;
    if &buf[..4] != LUA_SIGNATURE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unrecognized Lua signature {:x?}", &buf[..3]),
        ));
    }

    // Read and map version
    let version = r.read_u8().map_err(|e| field_error(e, "version"))?;
    let version = match version {
        0x51 => Version::Lua51,
        0x53 => Version::Lua53,
        _ => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported version {:x}", version),
        )),
    };

    let format = r.read_u8().map_err(|e| field_error(e, "format"))?;
    if format != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported format {:x}", format),
        ));
    }

    if version == Version::Lua53 {
        r.read_exact(&mut buf[..6]).map_err(|e| field_error(e, "test_data"))?;
        if &buf[..6] != &DATA[..] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unrecognized lua 5.3 test data: {:x?}", &buf[..6]),
            ));
        }
    }

    let mut endianness: LuaEndianness = LuaEndianness::Big;

    if version == Version::Lua51 {
        let e = r.read_u8().map_err(|e| field_error(e, "endianness"))?;
        match e {
            0 => endianness = LuaEndianness::Big,
            1 => endianness = LuaEndianness::Little,
            _ => return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported endianness {:x}", e),
            )),
        }
    }

    let int_bytes = match r.read_u8().map_err(|e| field_error(e, "int_bytes"))? {
        4 => ValueSize::Four,
        8 => ValueSize::Eight,
        v => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported int_bytes value size {}", v),
        )),
    };
    dbg!(int_bytes);
    let size_t_bytes = match r.read_u8().map_err(|e| field_error(e, "size_t_bytes"))? {
        4 => ValueSize::Four,
        8 => ValueSize::Eight,
        v => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported size_t_bytes value size {}", v),
        )),
    };
    dbg!(size_t_bytes);
    let inst_bytes = match r.read_u8().map_err(|e| field_error(e, "inst_bytes"))? {
        4 => ValueSize::Four,
        8 => ValueSize::Eight,
        v => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported inst_bytes value size {}", v),
        )),
    };
    dbg!(int_bytes);
    let mut integer_bytes = ValueSize::Eight;
    if version != Version::Lua51 {
        integer_bytes = match r.read_u8().map_err(|e| field_error(e, "integer_bytes"))? {
            4 => ValueSize::Four,
            8 => ValueSize::Eight,
            v => return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported integer_bytes value size {}", v),
            )),
        };
    }
    dbg!(integer_bytes);
    let num_bytes = match r.read_u8().map_err(|e| field_error(e, "num_bytes"))? {
        4 => ValueSize::Four,
        8 => ValueSize::Eight,
        v => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported num_bytes value size {}", v),
        )),
    };
    dbg!(num_bytes);

    // 52/53 endianness detection
    if version == Version::Lua53 {
        // why did puc-rio switch to this? so annoying...
        let mut endian = LuaEndianness::Big;
        let mut test_int = match integer_bytes {
            ValueSize::Four => {
                r.read_i32::<BE>().map(|v| v as i64)
            },
            ValueSize::Eight => {
                r.read_i64::<BE>()
            },
        }.map_err(|e| field_error(e, "test_int"))?;
        let mut test_num = match num_bytes {
            ValueSize::Four => {
                r.read_f32::<BE>().map(|v| v as f64)
            },
            ValueSize::Eight => {
                r.read_f64::<BE>()
            },
        }.map_err(|e| field_error(e, "test_num"))?;

        if test_int != TEST_INT {
            test_int = test_int.to_le();
            test_num = f64::from_le_bytes(test_num.to_le_bytes());
            endian = LuaEndianness::Little;

            if test_num != TEST_NUMBER {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unrecognized test integer",
                ));
            }
        }

        if test_num.abs() - TEST_NUMBER > 0.0001 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unrecognized test number",
            ));
        }

        endianness = endian;
    }
    let integral_flag;
    if version == Version::Lua51 {
        integral_flag = r.read_u8().map_err(|e| field_error(e, "integral_flag"))? == 1;
    } else {
        integral_flag = false;
    }

    Ok(ChunkHeader {
        version,
        endian: endianness,
        int_bytes,
        size_bytes: size_t_bytes,
        inst_bytes,
        lua_integer_bytes: integer_bytes,
        lua_number_bytes: num_bytes,
        integral_flag,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    use std::io::Cursor;

    static LUAC51_BYTES: &'static [u8] = include_bytes!("test/test51le.luac");

    #[test]
    fn test_header_51() {
        let header = read_header(Cursor::new(LUAC51_BYTES)).unwrap();
        assert_eq!(header, ChunkHeader {
            version: Version::Lua51,
            endian: LuaEndianness::Little,
            int_bytes: ValueSize::Four,
            size_bytes: ValueSize::Four,
            inst_bytes: ValueSize::Four,
            lua_integer_bytes: ValueSize::Eight,
            lua_number_bytes: ValueSize::Eight,
            integral_flag: false,
        });
    }
}
