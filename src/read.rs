//! Reading Lua Binary Chunks from IO sources.

use crate::types::{
    Chunk,
    ChunkHeader,
    Constant,
    LuaDebug,
    LuaDebugLocalVar,
    LuaEndianness,
    LuaInteger,
    LuaNumber,
    LuaInstruction,
    Prototype,
    ValueSize,
    Version,
};

use std::convert::TryInto;
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
    let prototype = match header.endian {
        LuaEndianness::Little => {
            let mut lr = LuaReader::<_, LE>::new(
                &mut r,
                header.clone(),
            );
            lr.read_prototype()?
        },
        LuaEndianness::Big => {
            let mut lr = LuaReader::<_, BE>::new(
                &mut r,
                header.clone(),
            );
            lr.read_prototype()?
        },
    };

    Ok(Chunk {
        header,
        proto: prototype,
    })
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
            format!("Unsupported version 0x{:x}", version),
        )),
    };

    let format = r.read_u8().map_err(|e| field_error(e, "format"))?;
    if format != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported format 0x{:x}", format),
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
                format!("Unsupported endianness 0x{:x}", e),
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
    let size_t_bytes = match r.read_u8().map_err(|e| field_error(e, "size_t_bytes"))? {
        4 => ValueSize::Four,
        8 => ValueSize::Eight,
        v => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported size_t_bytes value size {}", v),
        )),
    };
    let inst_bytes = match r.read_u8().map_err(|e| field_error(e, "inst_bytes"))? {
        4 => ValueSize::Four,
        8 => ValueSize::Eight,
        v => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported inst_bytes value size {}", v),
        )),
    };
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
    let num_bytes = match r.read_u8().map_err(|e| field_error(e, "num_bytes"))? {
        4 => ValueSize::Four,
        8 => ValueSize::Eight,
        v => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported num_bytes value size {}", v),
        )),
    };

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
            test_int = i64::from_be_bytes(test_int.to_le_bytes());
            test_num = f64::from_be_bytes(test_num.to_le_bytes());
            endian = LuaEndianness::Little;

            if test_int != TEST_INT {
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



struct LuaReader<R: Read, E: ByteOrder> {
    r: R,
    header: ChunkHeader,
    _pd: std::marker::PhantomData<E>,
}

impl<R, E> LuaReader<R, E>
where
    R: Read,
    E: ByteOrder,
{
    pub fn new(r: R, header: ChunkHeader) -> Self {
        LuaReader {
            r,
            header,
            _pd: std::marker::PhantomData,
        }
    }

    pub fn read_lua_vector<T, F>(&mut self, mut f: F) -> io::Result<Vec<T>>
    where
        F: FnMut(&mut LuaReader<R, E>) -> io::Result<T>,
    {
        let size = self.read_lua_int()?;
        if size == 0 {
            Ok(Vec::new())
        } else {
            let safe_size: usize = size.try_into().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("can't parse lua vec of size {}: {}", size, e),
                )
            })?;
            let mut ret = Vec::with_capacity(safe_size);
            for _ in 0..safe_size {
                let t = (f)(self)?;
                ret.push(t);
            }
            Ok(ret)
        }
    }

    pub fn read_lua_string(&mut self) -> io::Result<Vec<u8>> {
        match self.header.version {
            Version::Lua51 => self.read_lua_string_51(),
            Version::Lua53 => self.read_lua_string_52(),
        }
    }

    fn read_lua_string_51(&mut self) -> io::Result<Vec<u8>> {
        let size = self.read_lua_size_t()?;
        if size == 0 {
            Ok(Vec::new())
        } else {
            let safe_size: usize = size.try_into().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("can't parse lua 51 string of size {}: {}", size, e),
                )
            })?;
            let mut buffer = vec![0u8; safe_size];
            self.r.read_exact(&mut buffer[..])?;
            Ok(buffer)
        }
    }

    fn read_lua_string_52(&mut self) -> io::Result<Vec<u8>> {
        let small_size = self.r.read_u8()?;
        if small_size == 0 {
            Ok(Vec::new())
        } else {
            let len = if small_size < 0xFF {
                small_size as usize
            } else {
                let size = self.read_lua_size_t()?;
                let safe_size: usize = size.try_into().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("can't parse lua 51 string of size {}: {}", size, e),
                    )
                })?;
                safe_size
            };
            let mut buffer = vec![0u8; len];
            self.r.read_exact(&mut buffer[..])?;
            Ok(buffer)
        }
    }

    pub fn read_lua_int(&mut self) -> io::Result<i64> {
        let integer = match self.header.int_bytes {
            ValueSize::Four => self.r.read_i32::<E>()? as i64,
            ValueSize::Eight => self.r.read_i64::<E>()? as i64,
        };
        Ok(integer)
    }

    pub fn read_lua_size_t(&mut self) -> io::Result<i64> {
        let integer = match self.header.size_bytes {
            ValueSize::Four => self.r.read_i32::<E>()? as i64,
            ValueSize::Eight => self.r.read_i64::<E>()? as i64,
        };
        Ok(integer)
    }

    pub fn read_lua_integer(&mut self) -> io::Result<LuaInteger> {
        let integer = match self.header.lua_integer_bytes {
            ValueSize::Four => self.r.read_i32::<E>()? as LuaInteger,
            ValueSize::Eight => self.r.read_i64::<E>()? as LuaInteger,
        };
        Ok(integer)
    }

    pub fn read_lua_number(&mut self) -> io::Result<LuaNumber> {
        let integer = match self.header.lua_number_bytes {
            ValueSize::Four => self.r.read_f32::<E>()? as LuaNumber,
            ValueSize::Eight => self.r.read_f64::<E>()? as LuaNumber,
        };
        Ok(integer)
    }

    pub fn read_lua_instruction(&mut self) -> io::Result<LuaInstruction> {
        let instruction = match self.header.inst_bytes {
            ValueSize::Four => self.r.read_u32::<E>()? as u64,
            ValueSize::Eight => self.r.read_u64::<E>()? as u64,
        };
        Ok(instruction)
    }

    pub fn read_prototype(&mut self) -> io::Result<Prototype> {
        match self.header.version {
            Version::Lua51 => self.read_prototype51(),
            Version::Lua53 => self.read_prototype53()
        }
    }

    fn read_prototype51(&mut self) -> io::Result<Prototype> {
        let source = self.read_lua_string().map_err(|e| field_error(e, "source"))?;
        let line_defined = self.read_lua_int().map_err(|e| field_error(e, "line_defined"))?;
        let last_line_defined = self.read_lua_int().map_err(|e| field_error(e, "last_line_defined"))?;
        let nups = self.read_u8().map_err(|e| field_error(e, "nups"))?;
        let num_params = self.read_u8().map_err(|e| field_error(e, "num_params"))?;
        let is_vararg = self.read_u8().map_err(|e| field_error(e, "is_vararg"))?;
        let max_stack_size = self.read_u8().map_err(|e| field_error(e, "max_stack_size"))?;
        let code = self.read_lua_vector(|lr| lr.read_lua_instruction()).map_err(|e| field_error(e, "code"))?;
        let constants = self.read_lua_vector(|lr| {
            let c = match lr.read_u8()? {
                0x00 => Constant::Nil,
                0x01 => Constant::Boolean(lr.read_u8()? > 0),
                0x03 => Constant::Number(lr.read_lua_number()?),
                0x13 => Constant::Integer(lr.read_lua_integer()?),
                0x04 => Constant::String(lr.read_lua_string()?),
                0x14 => Constant::String(lr.read_lua_string()?),
                o => return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unrecognized constant type {}", o),
                )),
            };
            Ok(c)
        }).map_err(|e| field_error(e, "constants"))?;
        // let upvalues = self.read_lua_vector(|lr| {
        //     let stack = lr.read_u8()?;
        //     let idx = lr.read_u8()?;
        //     Ok(match stack {
        //         0 => Upvalue::Outer(idx),
        //         _ => Upvalue::Stack(idx),
        //     })
        // }).map_err(|e| field_error(e, "upvalues"))?;
        let protos = self.read_lua_vector(|lr| {
            lr.read_prototype51()
        }).map_err(|e| field_error(e, "protos"))?;
        let debug = self.read_lua_debug().map_err(|e| field_error(e, "debug"))?;

        Ok(Prototype {
            source,
            line_defined,
            last_line_defined,
            num_params,
            is_vararg,
            max_stack_size,
            code,
            constants,
            upvalues: Vec::new(),
            nups,
            protos,
            debug,
        })
    }

    fn read_prototype53(&mut self) -> io::Result<Prototype> {
        todo!()
    }

    pub fn read_lua_debug(&mut self) -> io::Result<LuaDebug> {
        let lineinfo = self.read_lua_vector(|lr| {
            lr.read_lua_int()
        }).map_err(|e| field_error(e, "lineinfo"))?;
        let localvars = self.read_lua_vector(|lr| {
            let name = lr.read_lua_string()?;
            let start_pc = lr.read_lua_int()?;
            let end_pc = lr.read_lua_int()?;
            Ok(LuaDebugLocalVar {
                name,
                start_pc,
                end_pc,
            })
        }).map_err(|e| field_error(e, "localvars"))?;
        let upvalues = self.read_lua_vector(|lr| {
            lr.read_lua_string()
        }).map_err(|e| field_error(e, "upvalues"))?;

        Ok(LuaDebug {
            lineinfo,
            localvars,
            upvalues,
        })
    }
}

impl<R, E> Read for LuaReader<R, E>
where
    R: Read,
    E: ByteOrder,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.r.read(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::io::Cursor;

    static LUAC51_BYTES: &'static [u8] = include_bytes!("test/test51le.luac");
    static LUAC53_BYTES: &'static [u8] = include_bytes!("test/test53le.luac");

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

    #[test]
    fn test_header_53() {
        let header = read_header(Cursor::new(LUAC53_BYTES)).unwrap();
        assert_eq!(header, ChunkHeader {
            version: Version::Lua53,
            endian: LuaEndianness::Little,
            int_bytes: ValueSize::Four,
            size_bytes: ValueSize::Four,
            inst_bytes: ValueSize::Four,
            lua_integer_bytes: ValueSize::Eight,
            lua_number_bytes: ValueSize::Eight,
            integral_flag: false,
        });
    }

    #[test]
    fn test_chunk_51_le() {
        let chunk = read_chunk(Cursor::new(LUAC51_BYTES)).unwrap();
        println!("{:#?}", chunk);
        let mut out = Vec::new();
        crate::write::write_chunk(&mut out, &chunk).unwrap();
        assert_eq!(&LUAC51_BYTES[..], &out[..]);
    }
}
