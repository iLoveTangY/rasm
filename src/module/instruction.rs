pub mod instruction {
    use crate::module::OpCode;
    use std::any::Any;
    use std::fmt;

    pub struct Instruction {
        pub opcode: OpCode,
        pub args: Option<Box<dyn Any>>,
    }

    impl Instruction {
        pub fn get_op_name(&self) -> String {
            self.opcode.to_string()
        }
    }

    pub struct MemArg {
        pub align: u32,
        pub offset: u32,
    }

    impl fmt::Display for MemArg {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "align: {}, offset: {}", self.align, self.offset)
        }
    }

    pub type BlockType = i32;

    pub const BLOCK_TYPE_I32: BlockType = -1;
    pub const BLOCK_TYPE_I64: BlockType = -2;
    pub const BLOCK_TYPE_F32: BlockType = -3;
    pub const BLOCK_TYPE_F64: BlockType = -4;
    pub const BLOCK_TYPE_EMPTY: BlockType = -64;

    pub struct BlockArgs {
        pub block_type: BlockType,
        pub instructions: Vec<Instruction>,
    }

    pub struct IfArgs {
        pub block_type: BlockType, // block 的返回值类型
        pub instructions_1: Vec<Instruction>,
        pub instructions_2: Vec<Instruction>,
    }

    type LabelIdx = u32;
    pub struct BrTableArgs {
        pub labels: Vec<LabelIdx>,
        pub default: LabelIdx,
    }

    impl fmt::Display for BrTableArgs {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "labels: {:?}, default: {}", self.labels, self.default)
        }
    }
}
