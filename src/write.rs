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
	try!(writer.write_header());
	try!(writer.out.write_u8(function.upvalues.len() as u8));
	writer.write_function(function)
}

struct Writer<W: Write> {
	out: W,
}

impl<W: Write> Writer<W> {
	fn write_header(&mut self) -> io::Result<()> {
		try!(self.out.write_all(SIGNATURE));
		try!(self.out.write_u8(VERSION));
		try!(self.out.write_u8(FORMAT));
		try!(self.out.write_all(DATA));
		try!(self.out.write_u8(size_of::<Int>() as u8));
		try!(self.out.write_u8(size_of::<Size>() as u8));
		try!(self.out.write_u8(size_of::<Instruction>() as u8));
		try!(self.out.write_u8(size_of::<Integer>() as u8));
		try!(self.out.write_u8(size_of::<Number>() as u8));
		try!(self.out.write_i64::<E>(TEST_INT));
		try!(self.out.write_f64::<E>(TEST_NUMBER));
		Ok(())
	}

	fn write_function(&mut self, function: &Function) -> io::Result<()> {
		try!(self.write_string(&function.source));
		try!(self.out.write_i32::<E>(function.line_start));
		try!(self.out.write_i32::<E>(function.line_end));
		try!(self.out.write_u8(function.num_params));
		try!(self.out.write_u8(if function.is_vararg { 1 } else { 0 }));
		try!(self.out.write_u8(function.max_stack_size));

		try!(self.out.write_u32::<E>(function.code.len() as u32));
		for &ins in &function.code {
			try!(self.out.write_u32::<E>(ins));
		}
		try!(self.out.write_u32::<E>(function.constants.len() as u32));
		for cons in &function.constants {
			match cons {
				&Constant::Nil => try!(self.out.write_u8(0x00)),
				&Constant::Boolean(b) => try!(self.out.write_all(&[0x01, if b { 1 } else { 0 }])),
				&Constant::Float(n) => {
					try!(self.out.write_u8(0x03));
					try!(self.out.write_f64::<E>(n));
				}
				&Constant::Int(n) => {
					try!(self.out.write_u8(0x13));
					try!(self.out.write_i64::<E>(n));
				}
				&Constant::ShortString(ref s) => {
					try!(self.out.write_u8(0x04));
					try!(self.write_string(s));
				}
				&Constant::LongString(ref s) => {
					try!(self.out.write_u8(0x14));
					try!(self.write_string(s));
				}
			}
		}
		try!(self.out.write_u32::<E>(function.upvalues.len() as u32));
		for upval in &function.upvalues {
			try!(match upval {
				&Upvalue::Outer(idx) => self.out.write_all(&[0, idx]),
				&Upvalue::Stack(idx) => self.out.write_all(&[1, idx]),
			});
		}
		try!(self.out.write_u32::<E>(function.protos.len() as u32));
		for proto in &function.protos {
			try!(self.write_function(proto));
		}
		// debug
		try!(self.out.write_u32::<E>(function.debug.lineinfo.len() as u32));
		for &line in &function.debug.lineinfo {
			try!(self.out.write_i32::<E>(line));
		}
		try!(self.out.write_u32::<E>(function.debug.localvars.len() as u32));
		for var in &function.debug.localvars {
			try!(self.write_string(&var.name));
			try!(self.out.write_i32::<E>(var.start_pc));
			try!(self.out.write_i32::<E>(var.end_pc));
		}
		try!(self.out.write_u32::<E>(function.debug.upvalues.len() as u32));
		for upval in &function.debug.upvalues {
			try!(self.write_string(upval));
		}
		Ok(())
	}

	fn write_string(&mut self, string: &str) -> io::Result<()> {
		if string.len() >= 0xff {
			try!(self.out.write_u8(0xff));
			try!(self.out.write_u32::<E>(string.len() as u32 + 1));
		} else {
			try!(self.out.write_u8(string.len() as u8 + 1));
		}
		self.out.write_all(string.as_bytes())
	}
}
