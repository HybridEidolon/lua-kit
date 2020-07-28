//! Writing Lua Binary Chunks from IO sources.

use crate::types::{
    Chunk,
    ChunkHeader,
    Constant,
    LuaDebug,
    LuaEndianness,
    LuaInteger,
    LuaNumber,
    LuaInstruction,
    Prototype,
    ValueSize,
    Version,
};

use std::convert::TryInto;
use std::io::{self, Write};

use byteorder::{BE, LE, ByteOrder, WriteBytesExt};


// common binary chunk signature
const LUA_SIGNATURE: &'static [u8] = b"\x1bLua";
// used by lua 5.3
const DATA: &'static [u8] = b"\x19\x93\r\n\x1a\n";
// A test integer to know endianness.
const TEST_INT: i64 = 0x5678;
const TEST_INT_I32: i32 = 0x5678i32;
// A test floating-point number to know endianness.
const TEST_NUMBER: f64 = 370.5;
const TEST_NUMBER_F32: f32 = 370.5f32;

pub fn write_chunk<W: Write>(mut w: W, chunk: &Chunk) -> io::Result<()> {
    write_header(&mut w, &chunk.header)?;
    match chunk.header.endian {
        LuaEndianness::Little => {
            let mut lw = LuaWriter::<_, LE>::new(&mut w, chunk.header.clone());
            lw.write_prototype(&chunk.proto)?;
        },
        LuaEndianness::Big => {
            let mut lw = LuaWriter::<_, BE>::new(&mut w, chunk.header.clone());
            lw.write_prototype(&chunk.proto)?;
        },
    }
    Ok(())
}

fn write_header<W: Write>(mut w: W, header: &ChunkHeader) -> io::Result<()> {
    w.write_all(LUA_SIGNATURE)?;
    w.write_u8(header.version as u8)?;
    w.write_u8(0)?; // format

    if header.version == Version::Lua53 {
        w.write_all(DATA)?;
    }
    if header.version == Version::Lua51 {
        let endian = match header.endian {
            LuaEndianness::Little => 1,
            LuaEndianness::Big => 0,
        };
        w.write_u8(endian)?;
    }
    w.write_u8(header.int_bytes as u8)?;
    w.write_u8(header.size_bytes as u8)?;
    w.write_u8(header.inst_bytes as u8)?;
    if header.version == Version::Lua53 {
        w.write_u8(header.lua_integer_bytes as u8)?;
    }
    w.write_u8(header.lua_number_bytes as u8)?;
    w.write_u8(if header.integral_flag { 1 } else { 0 })?;

    if header.version == Version::Lua53 {
        match (header.endian, header.lua_integer_bytes) {
            (LuaEndianness::Little, ValueSize::Four) => {
                w.write_i32::<LE>(TEST_INT_I32)?;
            },
            (LuaEndianness::Little, ValueSize::Eight) => {
                w.write_i64::<LE>(TEST_INT)?;
            },
            (LuaEndianness::Big, ValueSize::Four) => {
                w.write_i32::<BE>(TEST_INT_I32)?;
            },
            (LuaEndianness::Big, ValueSize::Eight) => {
                w.write_i64::<BE>(TEST_INT)?;
            },
        }
        match (header.endian, header.lua_number_bytes) {
            (LuaEndianness::Little, ValueSize::Four) => {
                w.write_f32::<LE>(TEST_NUMBER_F32)?;
            },
            (LuaEndianness::Little, ValueSize::Eight) => {
                w.write_f64::<LE>(TEST_NUMBER)?;
            },
            (LuaEndianness::Big, ValueSize::Four) => {
                w.write_f32::<BE>(TEST_NUMBER_F32)?;
            },
            (LuaEndianness::Big, ValueSize::Eight) => {
                w.write_f64::<BE>(TEST_NUMBER)?;
            },
        }
    }

    Ok(())
}

struct LuaWriter<W: Write, E: ByteOrder> {
    w: W,
    header: ChunkHeader,
    _pd: std::marker::PhantomData<E>,
}


impl<W, E> LuaWriter<W, E>
where
    W: Write,
    E: ByteOrder,
{
    pub fn new(w: W, header: ChunkHeader) -> Self {
        LuaWriter {
            w,
            header,
            _pd: std::marker::PhantomData,
        }
    }

    pub fn write_prototype(&mut self, proto: &Prototype) -> io::Result<()> {
        match self.header.version {
            Version::Lua51 => self.write_prototype51(proto),
            Version::Lua53 => self.write_prototype53(proto),
        }
    }

    fn write_prototype51(&mut self, proto: &Prototype) -> io::Result<()> {
        self.write_lua_string(&proto.source[..])?;
        self.write_lua_int(proto.line_defined)?;
        self.write_lua_int(proto.last_line_defined)?;
        self.write_u8(proto.nups)?;
        self.write_u8(proto.num_params)?;
        self.write_u8(proto.is_vararg)?;
        self.write_u8(proto.max_stack_size)?;
        self.write_lua_int(proto.code.len() as i64)?;
        for c in proto.code.iter() {
            self.write_lua_instruction(*c)?;
        }
        self.write_lua_int(proto.constants.len() as i64)?;
        for con in proto.constants.iter() {
            match con {
                Constant::Nil => {
                    self.write_u8(0x00)?;
                },
                Constant::Boolean(v) => {
                    self.write_u8(0x01)?;
                    self.write_u8(if *v { 1 } else { 0 })?;
                },
                Constant::Number(v) => {
                    self.write_u8(0x03)?;
                    self.write_lua_number(*v)?;
                },
                Constant::Integer(v) => {
                    self.write_u8(0x13)?;
                    self.write_lua_integer(*v)?;
                },
                Constant::String(b) => {
                    if b.len() >= 0xFF {
                        self.write_u8(0x14)?;
                    } else {
                        self.write_u8(0x04)?;
                    }

                    self.write_lua_string(&b[..])?;
                },
            }
        }
        self.write_lua_int(proto.protos.len() as i64)?;
        for p in proto.protos.iter() {
            self.write_prototype51(p)?;
        }
        self.write_lua_debug(&proto.debug)?;

        Ok(())
    }

    fn write_prototype53(&mut self, _proto: &Prototype) -> io::Result<()> {
        todo!()
    }

    pub fn write_lua_string(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self.header.version {
            Version::Lua51 => self.write_lua_string_51(bytes),
            Version::Lua53 => self.write_lua_string_52(bytes),
        }
    }

    fn write_lua_string_51(&mut self, bytes: &[u8]) -> io::Result<()> {
        let safe_len: i64 = bytes.len().try_into().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unable to write lua51 string due to buffer size: {}", e),
            )
        })?;
        self.write_lua_size_t(safe_len)?;
        self.write_all(bytes)?;

        Ok(())
    }

    fn write_lua_string_52(&mut self, bytes: &[u8]) -> io::Result<()> {
        let safe_len: i64 = bytes.len().try_into().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unable to write lua51 string due to buffer size: {}", e),
            )
        })?;
        if safe_len < 0xFF {
            self.write_u8(safe_len as u8)?;
        } else {
            self.write_u8(0xFF)?;
            self.write_lua_size_t(safe_len)?;
        }
        self.write_all(bytes)?;

        Ok(())
    }

    pub fn write_lua_debug(&mut self, debug: &LuaDebug) -> io::Result<()> {
        self.write_lua_int(debug.lineinfo.len() as i64)?;
        for l in debug.lineinfo.iter() {
            self.write_lua_int(*l)?;
        }
        self.write_lua_int(debug.localvars.len() as i64)?;
        for v in debug.localvars.iter() {
            self.write_lua_string(&v.name[..])?;
            self.write_lua_int(v.start_pc)?;
            self.write_lua_int(v.end_pc)?;
        }
        self.write_lua_int(debug.upvalues.len() as i64)?;
        for v in debug.upvalues.iter() {
            self.write_lua_string(&v[..])?;
        }

        Ok(())
    }

    pub fn write_lua_int(&mut self, value: i64) -> io::Result<()> {
        match self.header.int_bytes {
            ValueSize::Four => self.write_i32::<E>(value as i32),
            ValueSize::Eight => self.write_i64::<E>(value),
        }
    }

    pub fn write_lua_size_t(&mut self, value: i64) -> io::Result<()> {
        match self.header.size_bytes {
            ValueSize::Four => self.write_i32::<E>(value as i32),
            ValueSize::Eight => self.write_i64::<E>(value),
        }
    }

    pub fn write_lua_integer(&mut self, value: LuaInteger) -> io::Result<()> {
        match self.header.lua_integer_bytes {
            ValueSize::Four => self.write_i32::<E>(value as i32),
            ValueSize::Eight => self.write_i64::<E>(value),
        }
    }

    pub fn write_lua_number(&mut self, value: LuaNumber) -> io::Result<()> {
        match self.header.lua_number_bytes {
            ValueSize::Four => self.write_f32::<E>(value as f32),
            ValueSize::Eight => self.write_f64::<E>(value),
        }
    }

    pub fn write_lua_instruction(&mut self, value: LuaInstruction) -> io::Result<()> {
        match self.header.inst_bytes {
            ValueSize::Four => self.write_u32::<E>(value as u32),
            ValueSize::Eight => self.write_u64::<E>(value),
        }
    }
}

impl<W, E> Write for LuaWriter<W, E>
where
    W: Write,
    E: ByteOrder,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.w.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}
