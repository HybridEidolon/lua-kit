//! Serialization code.

use std::io::{self, Write};
use std::mem::size_of;
use byteorder::WriteBytesExt;
use byteorder::NativeEndian as E;

use super::{
    SIGNATURE, FORMAT, VERSION, DATA, TEST_INT, TEST_NUMBER,
    Int, Size, Instruction, Integer, Number,
    Constant, Upvalue, Function,
};

/// Serialize a `Function` to bytecode.
pub fn write_file<W: Write>(write: W, function: &Function) -> io::Result<()> {
    let mut writer = Writer { out: write };
    writer.write_header()?;
    writer.out.write_u8(function.upvalues.len() as u8)?;
    writer.write_function(function)
}

struct Writer<W: Write> {
    out: W,
}

impl<W: Write> Writer<W> {
    fn write_header(&mut self) -> io::Result<()> {
        self.out.write_all(SIGNATURE)?;
        self.out.write_u8(VERSION)?;
        self.out.write_u8(FORMAT)?;
        self.out.write_all(DATA)?;
        self.out.write_u8(size_of::<Int>() as u8)?;
        self.out.write_u8(size_of::<Size>() as u8)?;
        self.out.write_u8(size_of::<Instruction>() as u8)?;
        self.out.write_u8(size_of::<Integer>() as u8)?;
        self.out.write_u8(size_of::<Number>() as u8)?;
        self.out.write_i64::<E>(TEST_INT)?;
        self.out.write_f64::<E>(TEST_NUMBER)?;
        Ok(())
    }

    fn write_function(&mut self, function: &Function) -> io::Result<()> {
        self.write_string(&function.source)?;
        self.out.write_i32::<E>(function.line_start)?;
        self.out.write_i32::<E>(function.line_end)?;
        self.out.write_u8(function.num_params)?;
        self.out.write_u8(if function.is_vararg { 1 } else { 0 })?;
        self.out.write_u8(function.max_stack_size)?;

        self.out.write_u32::<E>(function.code.len() as u32)?;
        for &ins in &function.code {
            self.out.write_u32::<E>(ins)?;
        }
        self.out.write_u32::<E>(function.constants.len() as u32)?;
        for cons in &function.constants {
            match cons {
                &Constant::Nil => self.out.write_u8(0x00)?,
                &Constant::Boolean(b) => self.out.write_all(&[0x01, if b { 1 } else { 0 }])?,
                &Constant::Float(n) => {
                    self.out.write_u8(0x03)?;
                    self.out.write_f64::<E>(n)?;
                }
                &Constant::Int(n) => {
                    self.out.write_u8(0x13)?;
                    self.out.write_i64::<E>(n)?;
                }
                &Constant::ShortString(ref s) => {
                    self.out.write_u8(0x04)?;
                    self.write_string(s)?;
                }
                &Constant::LongString(ref s) => {
                    self.out.write_u8(0x14)?;
                    self.write_string(s)?;
                }
            }
        }
        self.out.write_u32::<E>(function.upvalues.len() as u32)?;
        for upval in &function.upvalues {
            match upval {
                &Upvalue::Outer(idx) => self.out.write_all(&[0, idx]),
                &Upvalue::Stack(idx) => self.out.write_all(&[1, idx]),
            }?;
        }
        self.out.write_u32::<E>(function.protos.len() as u32)?;
        for proto in &function.protos {
            self.write_function(proto)?;
        }
        // debug
        self.out.write_u32::<E>(function.debug.lineinfo.len() as u32)?;
        for &line in &function.debug.lineinfo {
            self.out.write_i32::<E>(line)?;
        }
        self.out.write_u32::<E>(function.debug.localvars.len() as u32)?;
        for var in &function.debug.localvars {
            self.write_string(&var.name)?;
            self.out.write_i32::<E>(var.start_pc)?;
            self.out.write_i32::<E>(var.end_pc)?;
        }
        self.out.write_u32::<E>(function.debug.upvalues.len() as u32)?;
        for upval in &function.debug.upvalues {
            self.write_string(upval)?;
        }
        Ok(())
    }

    fn write_string(&mut self, string: &str) -> io::Result<()> {
        if string.len() >= 0xff {
            self.out.write_u8(0xff)?;
            self.out.write_u32::<E>(string.len() as u32 + 1)?;
        } else {
            self.out.write_u8(string.len() as u8 + 1)?;
        }
        self.out.write_all(string.as_bytes())
    }
}
