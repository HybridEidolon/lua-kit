//! Types for various concepts in Lua chunks.

use std::ffi::CString;

/// The `nil` type in Lua.
pub type LuaNil = ();

/// The `Boolean` type in Lua.
pub type LuaBoolean = bool;

/// The `Number` type in Lua.
pub type LuaNumber = f64;

/// The `Integer` type in Lua.
pub type LuaInteger = i64;

/// An instruction in a Lua binary chunk.
pub type LuaInstruction = i64;

/// An entry in the constant pool.
///
/// Constants written by this library will behave according to the architecture
/// definition used at write time; this representation is semantically
/// architecture-independent.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    /// The value `nil`.
    Nil,

    /// A boolean.
    Boolean(LuaBoolean),

    /// A floating-point Number. Up to 8-byte Numbers are supported by this
    /// library.
    Number(LuaNumber),

    /// An integer. Up to 8-byte Integers are supported by this library.
    Integer(LuaInteger),

    /// A string of byte-width characters which contains no internal NUL values
    /// and ends with a NUL terminator.
    ///
    /// Strings in Lua do not have any specific encoding, and instead abide by
    /// the conventions used in C. This means that there is a difference between
    /// an empty string (1 NUL byte) versus a string which does not exist at all
    /// (0 bytes) when encoded in a Lua binary chunk.
    String(CString),
}

/// An entry in the upvalue list of a binary chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upvalue {
    /// An upvalue inherited from the outer function's upvalues.
    Outer(u8),
    /// An upvalue in the outer function's registers.
    Stack(u8),
}

/// An entry in the local variable debug table of a binary chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalVar {
    /// The local variable's name.
    pub name: Option<CString>,
    /// The instruction at which the local variable is introduced.
    pub start_pc: LuaInteger,
    /// The instruction at which the local variable goes out of scope.
    pub end_pc: LuaInteger,
}

/// Optional debugging information for a function.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Debug {
    /// The line number of each bytecode instruction.
    pub lineinfo: Vec<LuaInteger>,

    /// The names and scopes of local variables.
    pub localvars: Vec<LocalVar>,

    /// The names of upvalues.
    pub upvalues: Vec<CString>,
}

/// A Lua function prototype.
///
/// Lua makes no guarantees about the validity of any given binary chunk, and
/// will naively execute anything given to it. Thus, you may make any changes to
/// to this structure, which can always be serialized.
#[derive(Clone, Debug, PartialEq)]
pub struct Prototype {
    /// The source filename of the function. May be empty.
    pub source: Option<CString>,

    /// The start line number of the function.
    pub line_start: LuaInteger,

    /// The end line number of the function.
    pub line_end: LuaInteger,

    /// The number of fixed parameters the function takes.
    pub num_params: u8,

    /// Whether the function accepts a variable number of arguments.
    pub is_vararg: bool,

    /// The number of registers needed by the function.
    pub max_stack_size: u8,

    /// The function's code.
    pub code: Vec<LuaInstruction>,

    /// The function's constant table.
    pub constants: Vec<Constant>,

    /// The upvalue information of the function.
    pub upvalues: Vec<Upvalue>,

    /// The function's contained function prototypes.
    pub protos: Vec<Prototype>,

    /// Debugging information for the function.
    pub debug: Option<Debug>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChunkHeader {
    /// The Lua version supported by this chunk.
    pub version: Version,

    /// The endianness of this chunk when serialized.
    pub endian: LuaEndianness,

    /// The size of the `int` type in this chunk's target VM.
    pub int_bytes: ValueSize,

    /// The size of the `size_t` type in this chunk's target VM.
    pub size_bytes: ValueSize,

    /// The size of an instruction in this chunk's target VM.
    pub inst_bytes: ValueSize,

    /// The size of the Integer type in this chunk's target VM.
    pub lua_integer_bytes: ValueSize,

    /// The size of the Number type in this chunk's target VM.
    pub lua_number_bytes: ValueSize,

    /// If true, Lua numbers are integral. Only applies for Lua 5.1.
    pub integral_flag: bool,
}

/// Representation of a complete Lua binary chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    /// The header of this chunk.
    pub header: ChunkHeader,

    /// The prototype representing the top-level function of this chunk
    pub proto: Prototype,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Version {
    Lua51 = 0x51,
    Lua53 = 0x53,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueSize {
    Four = 4,
    Eight = 8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LuaEndianness {
    Little,
    Big,
}
