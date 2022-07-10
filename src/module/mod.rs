pub mod instruction;
pub mod module;
pub mod opcodes;
pub use instruction::instruction::BlockArgs;
pub use instruction::instruction::BrTableArgs;
pub use instruction::instruction::IfArgs;
pub use instruction::instruction::Instruction;
pub use instruction::instruction::MemArg;
pub use instruction::instruction::{
    BlockType, BLOCK_TYPE_EMPTY, BLOCK_TYPE_F32, BLOCK_TYPE_F64,
    BLOCK_TYPE_I32, BLOCK_TYPE_I64,
};
pub use module::module::ExportDesc;
pub use module::module::Expr;
pub use module::module::ImportDesc;
pub use module::module::Module;
pub use module::module::WasmReader;
pub use opcodes::opcodes::OpCode;
