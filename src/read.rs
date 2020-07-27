//! Deserialization code.

use std::error::Error;
use std::io::{self, Read};
use std::mem::size_of;
use byteorder::ReadBytesExt;
use byteorder::NativeEndian as E;

use super::{
    SIGNATURE, FORMAT, VERSION, DATA, TEST_INT, TEST_NUMBER,
    Int, Size, Instruction, Integer, Number,
    Constant, Upvalue, LocalVar, Debug, Function,
};

/// Deserialize bytecode into a `Function`.
pub fn read_file<R: Read>(read: R) -> io::Result<Function> {
    let mut reader = Reader { out: read };
    reader.read_header()?;
    reader.out.read_u8()?; // discard upvals header
    reader.read_function()
}

struct Reader<R: Read> {
    out: R,
}

fn invalid<T, S: Into<Box<dyn Error + Send + Sync>>>(s: S) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidInput, s))
}

macro_rules! check {
    ($get:expr, $want:expr, $note:expr) => {{
        let get = $get;
        let want = $want;
        if get != want {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!(
                "invalid {}, expected {:?} but got {:?}",
                $note, want, get,
            )));
        }
    }}
}

impl<R: Read> Reader<R> {
    fn read_all(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut start = 0;
        let len = buf.len();
        while start < len {
            let n = self.out.read(&mut buf[start..])?;
            if n == 0 {
                return invalid("unexpected EOF");
            }
            start += n;
        }
        Ok(())
    }

    fn read_header(&mut self) -> io::Result<()> {
        let mut buffer = [0u8; 6];
        self.read_all(&mut buffer[..4])?;
        check!(&buffer[..4], SIGNATURE, "signature");
        check!(self.out.read_u8()?, VERSION, "version");
        check!(self.out.read_u8()?, FORMAT, "format");
        self.read_all(&mut buffer)?;
        check!(&buffer, DATA, "test data");
        check!(self.out.read_u8()?, size_of::<Int>() as u8, "sizeof(int)");
        check!(self.out.read_u8()?, size_of::<Size>() as u8, "sizeof(size_t)");
        check!(self.out.read_u8()?, size_of::<Instruction>() as u8, "sizeof(Instruction)");
        check!(self.out.read_u8()?, size_of::<Integer>() as u8, "sizeof(Integer)");
        check!(self.out.read_u8()?, size_of::<Number>() as u8, "sizeof(Number)");
        check!(self.out.read_f64::<E>()?, TEST_NUMBER, "test number");
        check!(self.out.read_i64::<E>()?, TEST_INT, "test integer");
        Ok(())
    }

    fn read_function(&mut self) -> io::Result<Function> {
        Ok(Function {
            source: self.read_string()?,
            line_start: self.out.read_i32::<E>()?,
            line_end: self.out.read_i32::<E>()?,
            num_params: self.out.read_u8()?,
            is_vararg: self.out.read_u8()? != 0,
            max_stack_size: self.out.read_u8()?,
            code: self.read_vec(|this| Ok(this.out.read_u32::<E>()?))?,
            constants: self.read_vec(|this| Ok(match this.out.read_u8()? {
                0x00 => Constant::Nil,
                0x01 => Constant::Boolean(this.out.read_u8()? != 0),
                0x03 => Constant::Float(this.out.read_f64::<E>()?),
                0x13 => Constant::Int(this.out.read_i64::<E>()?),
                0x04 => Constant::ShortString(this.read_string()?),
                0x14 => Constant::LongString(this.read_string()?),
                o => return invalid(format!("unknown constant type {}", o)),
            }))?,
            upvalues: self.read_vec(|this| {
                let stack = this.out.read_u8()?;
                let idx = this.out.read_u8()?;
                Ok(match stack {
                    0 => Upvalue::Outer(idx),
                    _ => Upvalue::Stack(idx),
                })
            })?,
            protos: self.read_vec(|this| this.read_function())?,
            debug: Debug {
                lineinfo: self.read_vec(|this| Ok(this.out.read_i32::<E>()?))?,
                localvars: self.read_vec(|this| Ok(LocalVar {
                    name: this.read_string()?,
                    start_pc: this.out.read_i32::<E>()?,
                    end_pc: this.out.read_i32::<E>()?,
                }))?,
                upvalues: self.read_vec(|this| this.read_string())?,
            },
        })
    }

    #[inline]
    fn read_vec<F, T>(&mut self, f: F) -> io::Result<Vec<T>>
    where F: Fn(&mut Self) -> io::Result<T>
    {
        let len = self.out.read_u32::<E>()?;
        (0..len).map(|_| f(self)).collect()
    }

    fn read_string(&mut self) -> io::Result<String> {
        let first = self.out.read_u8()?;
        if first == 0 {
            Ok(String::new())
        } else {
            let len = if first < 0xff {
                first as usize
            } else {
                self.out.read_u32::<E>()? as usize
            } - 1;
            let mut buffer = vec![0u8; len];
            self.read_all(&mut buffer)?;
            // TODO: May need to return a Vec<u8> rather than String
            match String::from_utf8(buffer) {
                Ok(s) => Ok(s),
                Err(_) => invalid("not utf8"),
            }
        }
    }
}
