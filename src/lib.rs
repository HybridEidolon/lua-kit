//! Toolkit for working with serialized Lua functions and bytecode.
//!
//! Synced to Lua 5.3.

// pub mod bytecode;
mod write;
mod read;
mod types;

pub use self::types::{
    LuaNil,
    LuaBoolean,
    LuaNumber,
    LuaInteger,
    LuaInstruction,
    Constant,
    Upvalue,
    LuaDebugLocalVar,
    LuaDebug,
    Prototype,
    ChunkHeader,
    Chunk,
    Version,
    ValueSize,
    LuaEndianness,
};

pub use self::read::read_chunk;
pub use self::write::write_chunk;
