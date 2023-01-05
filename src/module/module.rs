pub mod module {
    use crate::module::BrTableArgs;
    use crate::module::IfArgs;
    use crate::module::Instruction;
    use crate::module::MemArg;
    use crate::module::OpCode;
    use crate::module::{
        BlockArgs, BlockType, BLOCK_TYPE_EMPTY, BLOCK_TYPE_F32, BLOCK_TYPE_F64,
        BLOCK_TYPE_I32, BLOCK_TYPE_I64,
    };
    use num_enum::TryFromPrimitive;
    use std::any::Any;
    use std::fmt;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::Path;
    use std::rc::Rc;
    use std::{convert::TryInto, panic};

    type TypeIdx = u32;
    type FuncIdx = u32; // 函数索引空间由外部函数和内部函数共同构成
    type TableIdx = u32; // 目前wasm规范只能导入或者定义一个表，所以唯一有效索引只能是 0
    type MemIdx = u32; // 和表索引空间一样，唯一有效索引只能是 0
    type GlobalIdx = u32; // 同样，全局变量索引空间由外部全局变量和内部全局变量共同构成
    type LocalIdx = u32; // 局部变量索引由函数的参数和局部变量构成
    type LableIdx = u32;

    // WASM 中只有4种值类型，i32、i64、f32、f64 和一种函数类型
    #[derive(TryFromPrimitive, Clone, Copy)]
    #[repr(u8)]
    pub enum ValType {
        I32 = 0x7F,
        I64 = 0x7E,
        F32 = 0x7D,
        F64 = 0x7C,
        FuncRef = 0x70,
    }

    impl fmt::Display for ValType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                ValType::I32 => write!(f, "i32"),
                ValType::I64 => write!(f, "i64"),
                ValType::F32 => write!(f, "f32"),
                ValType::F64 => write!(f, "f64"),
                ValType::FuncRef => write!(f, "funcref"),
            }
        }
    }

    #[derive(Clone)]
    pub struct FuncType {
        pub params_types: Vec<ValType>, // 函数的参数
        pub result_types: Vec<ValType>, // 函数的返回值
    }

    impl FuncType {
        fn get_signature(&self) -> String {
            let mut signature = String::new();
            signature.push_str("(");
            signature.push_str(
                &self
                    .params_types
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<String>>()
                    .join(","),
            );
            signature.push_str(")->(");
            signature.push_str(
                &self
                    .result_types
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<String>>()
                    .join(","),
            );
            signature.push_str(")");
            signature
        }
    }

    impl fmt::Display for FuncType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.get_signature())
        }
    }

    // Limits 类型用于描述表的元素数量或者内存页数的上下限
    #[derive(Clone, Copy)]
    pub struct Limits {
        pub min: usize,
        pub max: Option<usize>,
    }

    impl fmt::Display for Limits {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{{ min: {}, max: {} }}", self.min, self.max.unwrap_or(0))
        }
    }

    // 内存每页最大大小和最大的页数
    pub const PAGE_SIZE: usize = 65536; // 64kB
    pub const MAX_PAGE_COUNT: usize = 65536; // 2^16
                                             // 内存类型只需描述内存的页数限制，定义成Limits的别名即可
    pub type MemType = Limits;

    // 表类型需要描述表的元素类型以及元素数量的限制。Wasm规范只定义了一种元素类型，即函数引用，不过已经有提案建议增加其他元素类型
    // 为了反映二进制格式，也为了便于以后扩展，我们还是给元素类型留好位置
    pub struct TableType {
        pub elem_type: ValType, // 目前只能是 ValType::FuncRef
        pub limits: Limits,
    }

    impl fmt::Display for TableType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "{{ elem_type: {}, limits: {} }}",
                self.elem_type, self.limits
            )
        }
    }

    #[derive(Clone, Copy)]
    pub struct GlobalType {
        pub val_type: ValType,
        pub mutable: bool,
    }

    impl fmt::Display for GlobalType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{{ type: {}, mut: {} }}", self.val_type, self.mutable)
        }
    }

    pub type Expr = Vec<Instruction>;

    pub struct Global {
        pub global_type: GlobalType,
        pub init_expr: Expr,
    }

    pub struct Import {
        pub module_name: String, // 要导入的模块名
        pub member_name: String, // 导入模块的成员名
        pub desc: ImportDesc,    // 具体描述信息
    }

    #[derive(TryFromPrimitive)]
    #[repr(u8)]
    pub enum ImportTag {
        Func = 0x00,
        Table = 0x01,
        Mem = 0x02,
        Global = 0x03,
    }

    pub enum ImportDesc {
        Func(TypeIdx),
        Table(TableType),
        Mem(MemType),
        Global(GlobalType),
    }

    pub struct Export {
        pub name: String,
        pub desc: ExportDesc,
    }

    pub enum ExportDesc {
        Func(u32),
        Table(u32),
        Mem(u32),
        Global(u32),
    }

    pub struct Elem {
        pub table: TableIdx, // 表索引（初始化哪张表），由于目前标准规定模块最多只能导入或者定义一张表，因此表索引必须为零
        pub offset: Expr,    // 表内偏移量（从哪里开始初始化）
        pub init: Vec<FuncIdx>, // 函数索引列表（给定的初始数据）
    }

    #[derive(Clone)]
    pub struct Code {
        pub locals: Vec<Locals>, // 所有局部变量
        pub expr: Expr,          // 函数字节码
    }

    impl Code {
        pub fn get_local_count(&self) -> u64 {
            let mut n = 0u64;
            for locals in &self.locals {
                n += locals.n as u64;
            }
            n
        }
    }

    #[derive(Clone)]
    pub struct Locals {
        pub n: u32, // 个数，局部变量是压缩存储的，连续多个相同类型的局部变量会被分为一组
        pub val_type: ValType, // 类型
    }

    pub struct Data {
        pub mem: MemIdx, // 内存索引（初始化哪个内存），由于标准规定模块最多只能导入或者定义一个内存，因此内存索引必须为零
        pub offset: Expr, // 内存内偏移量（从哪里开始初始化）
        pub init: Vec<u8>, // 初始化数据
    }

    pub struct CustomSec {
        pub name: String,
        pub bytes: Vec<u8>,
    }

    const MAGIC_NUMBER: u32 = 0x6d736100; // "\0asm"
    const VERSION: u32 = 0x00000001; // 1

    const SEC_CUSTOM_ID: u8 = 0x00;
    const SEC_TYPE_ID: u8 = 0x01;
    const SEC_IMPORT_ID: u8 = 0x02;
    const SEC_FUNC_ID: u8 = 0x03;
    const SEC_TABLE_ID: u8 = 0x04;
    const SEC_MEM_ID: u8 = 0x05;
    const SEC_GLOBAL_ID: u8 = 0x06;
    const SEC_EXPORT_ID: u8 = 0x07;
    const SEC_START_ID: u8 = 0x08;
    const SEC_ELEM_ID: u8 = 0x09;
    const SEC_CODE_ID: u8 = 0x0a;
    const SEC_DATA_ID: u8 = 0x0b;

    pub struct Module {
        pub magic: u32,                 // magic number
        pub version: u32,               // version
        pub custom_sec: Vec<CustomSec>, // 自定义段不参与模块语义，存放的是额外信息，例如函数名和局部变量名等调试信息或者扩展信息，即使完全忽略这些信息也不影响模块的执行
        pub type_sec: Vec<FuncType>, // 类型段（ID为1），列出了WASM模块用到的所有函数类型
        pub import_sec: Vec<Import>, // WASM模块中的所有导入项
        pub func_sec: Vec<TypeIdx>,  // 内部函数的签名在类型段中的索引
        pub table_sec: Vec<TableType>, // 模块内部定义的表，Wasm规范规定模块最多只能定义一张表，且元素类型必须为函数引用（编码为0x70）
        pub mem_sec: Vec<MemType>,
        pub global_sec: Vec<Global>,
        pub export_sec: Vec<Export>, // Wasm 模块中的所有导出项
        pub start_sec: Option<FuncIdx>,  // Wasm 模块中的起始函数
        pub elem_sec: Vec<Elem>, // 元素段，存放表初始化数据
        pub code_sec: Vec<Code>, // 代码段，存放函数的字节码以及对应的局部变量信息
        pub data_sec: Vec<Data>, // 数据段，存放内存初始化数据
    }

    impl Module {
        pub fn get_block_type(&self, block_type: BlockType) -> FuncType {
            match block_type {
                BLOCK_TYPE_I32 => FuncType {
                    params_types: vec![],
                    result_types: vec![ValType::I32],
                },
                BLOCK_TYPE_I64 => FuncType {
                    params_types: vec![],
                    result_types: vec![ValType::I64],
                },
                BLOCK_TYPE_F32 => FuncType {
                    params_types: vec![],
                    result_types: vec![ValType::F32],
                },
                BLOCK_TYPE_F64 => FuncType {
                    params_types: vec![],
                    result_types: vec![ValType::F64],
                },
                BLOCK_TYPE_EMPTY => FuncType {
                    params_types: vec![],
                    result_types: vec![],
                },
                _ => self.type_sec[block_type as usize].clone(),
            }
        }
    }

    // LEB128 无符号整数解码
    fn decode_var_uint(data: &[u8]) -> (u64, usize) {
        let mut result = 0u64;
        for (index, value) in data.iter().enumerate() {
            result |= ((*value as u64) & 0x7f) << (index * 7);
            if value & 0x80 == 0 {
                // 表示已经解码结束
                return (result, index + 1);
            }
        }
        panic!("unexpected end of LEB128");
    }

    // LEB128 有符号整数解码
    // size: 可以为32或者64，表示解码的整数的位数
    fn decode_var_int(data: &[u8], size: usize) -> (i64, usize) {
        let mut result = 0i64;
        for (index, value) in data.iter().enumerate() {
            result |= ((*value as i64) & 0x7f) << (index * 7);
            if value & 0x80 == 0 {
                // 如果符号位是 1 的话需要给符号位补 1
                if (index * 7) < size && (*value & 0x40) != 0 {
                    result |= -1 << ((index + 1) * 7);
                }
                return (result, index + 1);
            }
        }
        panic!("unexpected end of LEB128");
    }

    pub struct WasmReader<'a> {
        data: &'a [u8],
    }

    impl<'a> WasmReader<'a> {
        fn new(data: &'a [u8]) -> WasmReader {
            WasmReader { data }
        }

        fn read_byte(&mut self) -> u8 {
            let result = self.data[0];
            self.data = &self.data[1..];
            result
        }

        fn read_u32(&mut self) -> u32 {
            let (u32_bytes, rest) = self.data.split_at(4);
            self.data = rest;
            u32::from_ne_bytes(u32_bytes.try_into().unwrap())
        }

        fn read_f32(&mut self) -> f32 {
            let (f32_bytes, rest) = self.data.split_at(4);
            self.data = rest;
            f32::from_ne_bytes(f32_bytes.try_into().unwrap())
        }

        fn read_f64(&mut self) -> f64 {
            let (f64_bytes, rest) = self.data.split_at(8);
            self.data = rest;
            f64::from_ne_bytes(f64_bytes.try_into().unwrap())
        }

        fn read_var_u32(&mut self) -> u32 {
            let (n, w) = decode_var_uint(self.data);
            self.data = &self.data[w..];
            n as u32
        }

        fn read_var_i32(&mut self) -> i32 {
            let (n, w) = decode_var_int(self.data, 32);
            self.data = &self.data[w..];
            n as i32
        }

        fn read_var_i64(&mut self) -> i64 {
            let (n, w) = decode_var_int(self.data, 64);
            self.data = &self.data[w..];
            n
        }

        fn read_bytes(&mut self) -> Vec<u8> {
            let len = self.read_var_u32();
            let (bytes, rest) = self.data.split_at(len as usize);
            self.data = rest;
            bytes.to_vec()
        }

        fn read_name(&mut self) -> String {
            let bytes = self.read_bytes();
            String::from_utf8(bytes).unwrap()
        }

        fn remaining(&self) -> usize {
            self.data.len()
        }

        fn read_val_type(&mut self) -> ValType {
            let val_type: ValType = self.read_byte().try_into().unwrap();
            val_type
        }

        fn read_val_types(&mut self) -> Vec<ValType> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_val_type());
            }
            result
        }

        fn read_func_type(&mut self) -> FuncType {
            let tag = self.read_byte();
            if tag != 0x60 {
                panic!("invalid func type tag");
            }
            FuncType {
                params_types: self.read_val_types(),
                result_types: self.read_val_types(),
            }
        }

        fn read_import_desc(&mut self) -> ImportDesc {
            let tag: ImportTag = self.read_byte().try_into().unwrap();
            match tag {
                ImportTag::Func => ImportDesc::Func(self.read_var_u32()),
                ImportTag::Table => ImportDesc::Table(self.read_table_type()),
                ImportTag::Mem => ImportDesc::Mem(self.read_limits()),
                ImportTag::Global => {
                    ImportDesc::Global(self.read_global_type())
                }
            }
        }

        fn read_block_type(&mut self) -> BlockType {
            let block_type = self.read_var_i32();
            if block_type < 0 {
                match block_type {
                    BLOCK_TYPE_I32 | BLOCK_TYPE_I64 | BLOCK_TYPE_F32
                    | BLOCK_TYPE_F64 | BLOCK_TYPE_EMPTY => (),
                    _ => panic!("malformed block type: {}", block_type),
                }
            }
            block_type
        }

        fn read_block_args(&mut self) -> BlockArgs {
            let block_type = self.read_block_type();
            let (instructions, end) = self.read_instructions();
            if end != OpCode::End {
                panic!("invalid block end: {}", end);
            }
            BlockArgs {
                block_type,
                instructions,
            }
        }

        fn read_if_args(&mut self) -> IfArgs {
            let block_type = self.read_block_type();
            let (instructions_1, end) = self.read_instructions();
            let mut instructions_2 = Expr::new();
            if end == OpCode::Else {
                let (instructions, end) = self.read_instructions();
                if end != OpCode::End {
                    panic!("invalid block end: {}", end);
                }
                instructions_2 = instructions;
            }
            IfArgs {
                block_type,
                instructions_1,
                instructions_2,
            }
        }

        fn read_br_table_args(&mut self) -> BrTableArgs {
            BrTableArgs {
                labels: self.read_indices(),
                default: self.read_var_u32(),
            }
        }

        fn read_zero(&mut self) -> u8 {
            let b = self.read_byte();
            if b != 0 {
                panic!("zero flag expected, got {}", b);
            }
            b
        }

        fn read_call_indirect_args(&mut self) -> u32 {
            let type_idx = self.read_var_u32();
            self.read_zero();
            type_idx
        }

        fn read_mem_arg(&mut self) -> MemArg {
            MemArg {
                align: self.read_var_u32(),
                offset: self.read_var_u32(),
            }
        }

        fn read_args(&mut self, opcode: &OpCode) -> Option<Rc<dyn Any>> {
            match opcode {
                OpCode::Block | OpCode::Loop => {
                    Some(Rc::new(self.read_block_args()))
                }
                OpCode::If => Some(Rc::new(self.read_if_args())),
                OpCode::Br | OpCode::BrIf => Some(Rc::new(self.read_var_u32())), // label index
                OpCode::BrTable => Some(Rc::new(self.read_br_table_args())),
                OpCode::Call => Some(Rc::new(self.read_var_u32())), // function index
                OpCode::CallIndirect => {
                    Some(Rc::new(self.read_call_indirect_args()))
                }
                OpCode::LocalGet | OpCode::LocalSet | OpCode::LocalTee => {
                    Some(Rc::new(self.read_var_u32()))
                } // local index
                OpCode::GlobalGet | OpCode::GlobalSet => {
                    Some(Rc::new(self.read_var_u32()))
                } // global index
                OpCode::MemorySize | OpCode::MemoryGrow => {
                    Some(Rc::new(self.read_zero()))
                }
                OpCode::I32Const => Some(Rc::new(self.read_var_i32())),
                OpCode::I64Const => Some(Rc::new(self.read_var_i64())),
                OpCode::F32Const => Some(Rc::new(self.read_f32())),
                OpCode::F64Const => Some(Rc::new(self.read_f64())),
                OpCode::TruncSat => Some(Rc::new(self.read_byte())),
                _ => {
                    if *opcode >= OpCode::I32Load
                        && *opcode <= OpCode::I64Store32
                    {
                        return Some(Rc::new(self.read_mem_arg()));
                    }
                    None
                }
            }
        }

        fn read_instruction(&mut self) -> Instruction {
            let opcode: OpCode = self.read_byte().try_into().unwrap();
            let args = self.read_args(&opcode);
            Instruction { opcode, args }
        }

        fn read_instructions(&mut self) -> (Expr, OpCode) {
            let mut instructions = Expr::new();
            loop {
                let instr = self.read_instruction();
                if instr.opcode == OpCode::Else || instr.opcode == OpCode::End {
                    return (instructions, instr.opcode);
                }
                instructions.push(instr);
            }
        }

        fn read_expr(&mut self) -> Expr {
            let (instrs, end) = self.read_instructions();
            // 确保表达式以 end 结尾
            if end != OpCode::End {
                panic!("invalid end of expression: {}", end);
            }
            instrs
        }

        fn read_locals(&mut self) -> Locals {
            Locals {
                n: self.read_var_u32(),
                val_type: self.read_val_type(),
            }
        }

        fn read_locals_vec(&mut self) -> Vec<Locals> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_locals());
            }
            result
        }

        fn read_code(&mut self) -> Code {
            // 每个代码项的所有内容
            let code_data = self.read_bytes();
            let mut code_reader = WasmReader::new(&code_data);
            let code = Code {
                locals: code_reader.read_locals_vec(),
                expr: code_reader.read_expr(),
            };
            if code.get_local_count() >= (u32::MAX as u64) {
                panic!("local count overflow");
            }
            code
        }

        fn read_custom_sec(&mut self) -> CustomSec {
            let data = self.read_bytes();
            let mut reader = WasmReader::new(&data);
            CustomSec {
                name: reader.read_name(),
                bytes: reader.data.to_vec(),
            }
        }

        fn read_import(&mut self) -> Import {
            Import {
                module_name: self.read_name(),
                member_name: self.read_name(),
                desc: self.read_import_desc(),
            }
        }

        fn read_limits(&mut self) -> Limits {
            let tag = self.read_byte();
            let min = self.read_var_u32();
            let max = if tag == 0x00 {
                None
            } else {
                Some(self.read_var_u32() as usize)
            };
            Limits {
                min: min as usize,
                max,
            }
        }

        fn read_table_type(&mut self) -> TableType {
            let elem_type = self.read_val_type();
            match elem_type {
                ValType::FuncRef => TableType {
                    elem_type,
                    limits: self.read_limits(),
                },
                _ => panic!("invalid table element type"),
            }
        }

        fn read_global_type(&mut self) -> GlobalType {
            GlobalType {
                val_type: self.read_val_type(),
                mutable: self.read_byte() == 0x01,
            }
        }

        fn read_export(&mut self) -> Export {
            Export {
                name: self.read_name(),
                desc: self.read_export_desc(),
            }
        }

        fn read_export_desc(&mut self) -> ExportDesc {
            let tag = self.read_byte();
            let value = self.read_var_u32();
            match tag {
                0x00 => ExportDesc::Func(value),
                0x01 => ExportDesc::Table(value),
                0x02 => ExportDesc::Mem(value),
                0x03 => ExportDesc::Global(value),
                _ => panic!("invalid export desc tag: {}", tag),
            }
        }

        fn read_elem(&mut self) -> Elem {
            Elem {
                table: self.read_var_u32(),
                offset: self.read_expr(),
                init: self.read_indices(),
            }
        }

        fn read_indices(&mut self) -> Vec<u32> {
            let len = self.read_var_u32();
            let mut result = Vec::with_capacity(len as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_var_u32());
            }
            result
        }

        fn read_data(&mut self) -> Data {
            Data {
                mem: self.read_var_u32(),
                offset: self.read_expr(),
                init: self.read_bytes(),
            }
        }

        // 类型段解码
        fn read_type_sec(&mut self) -> Vec<FuncType> {
            let len = self.read_var_u32();
            let mut result = Vec::with_capacity(len as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_func_type());
            }
            result
        }

        // 导入段解码
        fn read_import_sec(&mut self) -> Vec<Import> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_import());
            }
            result
        }

        // 函数段解码
        fn read_func_sec(&mut self) -> Vec<FuncIdx> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                // 存储的是函数类型在类型段中的索引
                result.push(self.read_var_u32());
            }
            result
        }

        // 表段解码
        fn read_table_sec(&mut self) -> Vec<TableType> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_table_type());
            }
            result
        }

        // 内存段解码
        fn read_mem_sec(&mut self) -> Vec<MemType> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_limits());
            }
            result
        }

        // Global 段解码
        fn read_global_sec(&mut self) -> Vec<Global> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(Global {
                    global_type: self.read_global_type(),
                    init_expr: self.read_expr(),
                });
            }
            result
        }

        // 导出段解码
        fn read_export_sec(&mut self) -> Vec<Export> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_export());
            }
            result
        }

        // 起始段解码
        fn read_start_sec(&mut self) -> Option<FuncIdx> {
            Some(self.read_var_u32())
        }

        // 元素段解码
        fn read_elem_sec(&mut self) -> Vec<Elem> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_elem());
            }
            result
        }

        // 代码段解码
        fn read_code_sec(&mut self) -> Vec<Code> {
            let len = self.read_var_u32();
            let mut result = Vec::with_capacity(len as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_code());
            }
            result
        }

        // Data 段解码
        fn read_data_sec(&mut self) -> Vec<Data> {
            let mut result = Vec::with_capacity(self.read_var_u32() as usize);
            for _index in 0..result.capacity() {
                result.push(self.read_data());
            }
            result
        }

        fn read_module(&mut self) -> Module {
            let mut module = Module {
                magic: self.read_u32(),
                version: self.read_u32(),
                custom_sec: Vec::new(),
                type_sec: Vec::new(),
                import_sec: Vec::new(),
                func_sec: Vec::new(),
                table_sec: Vec::new(),
                mem_sec: Vec::new(),
                global_sec: Vec::new(),
                export_sec: Vec::new(),
                start_sec: None,
                elem_sec: Vec::new(),
                code_sec: Vec::new(),
                data_sec: Vec::new(),
            };
            println!("magic: {:x}", module.magic);
            println!("version: {}", module.version);
            self.read_sections(&mut module);
            module
        }

        fn read_sections(&mut self, module: &mut Module) {
            let mut prev_sec_id = 0u8;
            while self.remaining() > 0 {
                let sec_id = self.read_byte();
                if sec_id == SEC_CUSTOM_ID {
                    module.custom_sec.push(self.read_custom_sec());
                    continue;
                }
                if sec_id > SEC_DATA_ID || sec_id <= prev_sec_id {
                    panic!("invalid section id");
                }
                prev_sec_id = sec_id;
                let sec_len = self.read_var_u32();
                let reamaining_before_read = self.remaining();
                self.read_non_custom_sec(sec_id, module);
                // 检查实际读取的长度和声明的 sec_len 是否一致
                if reamaining_before_read != self.remaining() + sec_len as usize
                {
                    panic!("section length mismatch: {}", sec_id);
                }
            }
        }

        fn read_non_custom_sec(&mut self, sec_id: u8, module: &mut Module) {
            match sec_id {
                SEC_TYPE_ID => module.type_sec = self.read_type_sec(),
                SEC_IMPORT_ID => module.import_sec = self.read_import_sec(),
                SEC_FUNC_ID => module.func_sec = self.read_func_sec(),
                SEC_TABLE_ID => module.table_sec = self.read_table_sec(),
                SEC_MEM_ID => module.mem_sec = self.read_mem_sec(),
                SEC_GLOBAL_ID => module.global_sec = self.read_global_sec(),
                SEC_EXPORT_ID => module.export_sec = self.read_export_sec(),
                SEC_START_ID => module.start_sec = self.read_start_sec(),
                SEC_ELEM_ID => module.elem_sec = self.read_elem_sec(),
                SEC_CODE_ID => module.code_sec = self.read_code_sec(),
                SEC_DATA_ID => module.data_sec = self.read_data_sec(),
                _ => panic!("unknown section id: {}", sec_id),
            }
        }

        pub fn decode_file<T: AsRef<Path>>(
            file_name: T,
        ) -> std::io::Result<Module> {
            let mut file = File::open(file_name.as_ref())?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            let mut wasm_reader = WasmReader::new(&buf);
            Ok(wasm_reader.read_module())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_decode_var_uint() {
            let data = vec![
                0b1_0111111,
                0b1_0011111,
                0b1_0001111,
                0b1_0000111,
                0b1_0000011,
                0b0_0000001,
            ];
            assert_eq!(decode_var_uint(&data[5..]), (0b0000001, 1));
            assert_eq!(decode_var_uint(&data[4..]), (0b1_0000011, 2));
            assert_eq!(decode_var_uint(&data[3..]), (0b1_0000011_0000111, 3));
            assert_eq!(
                decode_var_uint(&data[2..]),
                (0b1_0000011_0000111_0001111, 4)
            );
            assert_eq!(
                decode_var_uint(&data[1..]),
                (0b1_0000011_0000111_0001111_0011111, 5)
            );
        }

        #[test]
        fn test_decode_var_int() {
            let data = vec![0b1_1000000, 0b1_0111011, 0b0_1111000];
            assert_eq!(decode_var_int(&data[..], 32), (-123456, 3));
        }

        #[test]
        fn test_reader() {
            let data = vec![
                0x01, 0x02, 0x03, 0x04, 0x05, 0x00, 0x00, 0xc0, 0x3f, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x3f, 0xE5, 0x8E,
                0x26, // https://en.wikipedia.org/wiki/LEB128#Unsigned_LEB128
                0xC0, 0xBB,
                0x78, // https://en.wikipedia.org/wiki/LEB128#Signed_LEB128
                0xC0, 0xBB, 0x78, 0x03, 0x01, 0x02, 0x03, 0x03, 0x66, 0x6f,
                0x6f,
            ];
            let mut reader = WasmReader::new(&data);
            assert_eq!(reader.read_byte(), 0x01);
            assert_eq!(reader.read_u32(), 0x05040302);
            assert_eq!(reader.read_f32(), 1.5);
            assert_eq!(reader.read_f64(), 1.5);
            assert_eq!(reader.read_var_u32(), 624485);
            assert_eq!(reader.read_var_i32(), -123456);
            assert_eq!(reader.read_var_i64(), -123456);
            assert_eq!(reader.read_bytes(), [0x01, 0x02, 0x03]);
            assert_eq!(reader.read_name(), "foo");
            assert_eq!(reader.remaining(), 0);
        }

        #[test]
        fn test_decode_wasm_file() {
            let module = WasmReader::decode_file("data/hw_rust.wasm").unwrap();
            assert_eq!(module.magic, MAGIC_NUMBER);
            assert_eq!(module.version, VERSION);
            assert_eq!(module.custom_sec.len(), 2);
            assert_eq!(module.type_sec.len(), 15);
            assert_eq!(module.import_sec.len(), 0);
            assert_eq!(module.func_sec.len(), 171);
            assert_eq!(module.table_sec.len(), 1);
            assert_eq!(module.mem_sec.len(), 1);
            assert_eq!(module.global_sec.len(), 4);
            assert_eq!(module.export_sec.len(), 5);
            assert_eq!(module.start_sec, None);
            assert_eq!(module.elem_sec.len(), 1);
            assert_eq!(module.code_sec.len(), 171);
            assert_eq!(module.data_sec.len(), 4);
        }
    }
}
