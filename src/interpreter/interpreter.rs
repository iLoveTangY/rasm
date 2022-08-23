pub mod interpreter {
    use std::any::Any;
    use std::cell::RefCell;

    use crate::module::*;

    struct OperandStack {
        slots: Vec<u64>,
    }

    impl OperandStack {
        fn new() -> OperandStack {
            OperandStack { slots: Vec::new() }
        }

        fn length(&self) -> usize {
            self.slots.len()
        }

        fn push_u64(&mut self, val: u64) {
            self.slots.push(val);
        }

        fn pop_u64(&mut self) -> u64 {
            self.slots.pop().unwrap()
        }

        fn push_i64(&mut self, val: i64) {
            self.slots.push(val as u64);
        }

        fn pop_i64(&mut self) -> i64 {
            self.slots.pop().unwrap() as i64
        }

        fn push_u32(&mut self, val: u32) {
            self.slots.push(val as u64);
        }

        fn pop_u32(&mut self) -> u32 {
            self.slots.pop().unwrap() as u32
        }

        fn push_i32(&mut self, val: i32) {
            self.slots.push(val as u64);
        }

        fn pop_i32(&mut self) -> i32 {
            self.slots.pop().unwrap() as i32
        }

        fn push_f32(&mut self, val: f32) {
            self.push_u32(u32::from_ne_bytes(val.to_ne_bytes()));
        }

        fn pop_f32(&mut self) -> f32 {
            f32::from_ne_bytes(u32::to_ne_bytes(self.pop_u32()))
        }

        fn push_f64(&mut self, val: f64) {
            self.slots.push(u64::from_ne_bytes(f64::to_ne_bytes(val)));
        }

        fn pop_f64(&mut self) -> f64 {
            f64::from_ne_bytes(u64::to_ne_bytes(self.slots.pop().unwrap()))
        }

        fn push_bool(&mut self, val: bool) {
            self.slots.push(val as u64);
        }

        fn pop_bool(&mut self) -> bool {
            self.slots.pop().unwrap() != 0
        }
    }

    struct Memory {
        mem_type: MemType,
        data: Vec<u8>,
    }

    impl Memory {
        fn new(mem_type: MemType) -> Memory {
            let min_page_size = mem_type.min;
            Memory { mem_type, data: vec![0; (min_page_size * PAGE_SIZE) as usize] }
        }

        /// 已分配内存的页数
        fn size(&self) -> usize {
            self.data.len() / (PAGE_SIZE as usize)
        }

        /// 增长内存, 返回增长前的内存页数
        fn grow(&mut self, n: usize) -> usize {
            let old_size = self.size();
            if n == 0 {
                return old_size;
            }
            let max_page_count = self.mem_type.max.unwrap_or(MAX_PAGE_COUNT);
            if old_size + n > max_page_count {
                return 0xFFFFFFFF;
            }
            self.data.extend(vec![0; n].iter());
            old_size
        }

        fn read(&self, offset: usize, buf: &mut [u8]) {
            self.check_offset(offset, buf.len());
            buf.copy_from_slice(&self.data[offset..]);
        }

        fn write(&mut self, offset: usize, data: &[u8]) {
            self.check_offset(offset, data.len());
            self.data[offset..].copy_from_slice(data);
        }

        fn check_offset(&self, offset: usize, length: usize) {
            if self.data.len() - length < offset {
                panic!("Memory out of bounds");
            }
        }
    }

    pub struct VM<'a> {
        operand_stack: RefCell<OperandStack>,
        module: &'a Module,
        memory: Memory,
    }

    impl<'a> VM<'a> {
        fn new(module: &Module) -> VM {
            let memory: Memory;
            if module.mem_sec.len() > 0 {
                memory = Memory::new(module.mem_sec[0]);
            } else {
                memory = Memory::new(MemType { min: 0, max: None });
            }
            let operand_stack = RefCell::new(OperandStack::new());
            VM {
                operand_stack,
                module,
                memory,
            }
        }

        pub fn exec_main(module: &Module) {
            let vm = VM::new(module);
            if let Some(start_sec_id) = module.start_sec {
                vm.exec_code(start_sec_id as usize - module.import_sec.len());
            } else {
                println!("No start sec!");
            }
        }

        fn exec_code(&self, idx: usize) {
            let code = &self.module.code_sec[idx];
            for instr in &code.expr {
                self.exec_instr(instr);
            }
        }

        fn exec_instr(&self, instr: &Instruction) {
            match instr.opcode {
                OpCode::Call => self.call(&instr.args),
                OpCode::Drop => self.drop_value(&instr.args),
                OpCode::Select => self.select(&instr.args),
                OpCode::I32Const => self.i32_const(&instr.args),
                OpCode::I64Const => self.i64_const(&instr.args),
                OpCode::F32Const => self.f32_const(&instr.args),
                OpCode::F64Const => self.f64_const(&instr.args),
                OpCode::I32Eqz => self.i32_eqz(&instr.args),
                OpCode::I64Eqz => self.i64_eqz(&instr.args),
                OpCode::I32Eq => self.i32_eq(&instr.args),
                OpCode::I32Ne => self.i32_neq(&instr.args),
                OpCode::I32LtS => self.i32_lts(&instr.args),
                OpCode::I32LtU => self.i32_ltu(&instr.args),
                OpCode::I32GtS => self.i32_gts(&instr.args),
                OpCode::I32GtU => self.i32_gtu(&instr.args),
                OpCode::I32LeS => self.i32_les(&instr.args),
                OpCode::I32LeU => self.i32_leu(&instr.args),
                OpCode::I32GeS => self.i32_ges(&instr.args),
                OpCode::I32GeU => self.i32_geu(&instr.args),
                OpCode::I64Eq => self.i64_eq(&instr.args),
                OpCode::I64Ne => self.i64_neq(&instr.args),
                OpCode::I64LtS => self.i64_lts(&instr.args),
                OpCode::I64LtU => self.i64_ltu(&instr.args),
                OpCode::I64GtS => self.i64_gts(&instr.args),
                OpCode::I64GtU => self.i64_gtu(&instr.args),
                OpCode::I64LeS => self.i64_les(&instr.args),
                OpCode::I64LeU => self.i64_leu(&instr.args),
                OpCode::I64GeS => self.i64_ges(&instr.args),
                OpCode::I64GeU => self.i64_geu(&instr.args),
                OpCode::F32Eq => self.f32_eq(&instr.args),
                OpCode::F32Ne => self.f32_neq(&instr.args),
                OpCode::F32Lt => self.f32_lt(&instr.args),
                OpCode::F32Gt => self.f32_gt(&instr.args),
                OpCode::F32Le => self.f32_le(&instr.args),
                OpCode::F32Ge => self.f32_ge(&instr.args),
                OpCode::F64Eq => self.f64_eq(&instr.args),
                OpCode::F64Ne => self.f64_neq(&instr.args),
                OpCode::F64Lt => self.f64_lt(&instr.args),
                OpCode::F64Gt => self.f64_gt(&instr.args),
                OpCode::F64Le => self.f64_le(&instr.args),
                OpCode::F64Ge => self.f64_ge(&instr.args),
                OpCode::I32Clz => self.i32_clz(&instr.args),
                OpCode::I32Ctz => self.i32_ctz(&instr.args),
                OpCode::I32PopCnt => self.i32_pop_cnt(&instr.args),
                OpCode::I64Clz => self.i64_clz(&instr.args),
                OpCode::I64Ctz => self.i64_ctz(&instr.args),
                OpCode::I64PopCnt => self.i64_pop_cnt(&instr.args),
                OpCode::F32Abs => self.f32_abs(&instr.args),
                OpCode::F32Neg => self.f32_neg(&instr.args),
                OpCode::F32Ceil => self.f32_ceil(&instr.args),
                OpCode::F32Floor => self.f32_floor(&instr.args),
                OpCode::F32Trunc => self.f32_trunc(&instr.args),
                OpCode::F32Nearest => self.f32_nearest(&instr.args),
                OpCode::F32Sqrt => self.f32_sqrt(&instr.args),
                OpCode::F64Abs => self.f64_abs(&instr.args),
                OpCode::F64Neg => self.f64_neg(&instr.args),
                OpCode::F64Ceil => self.f64_ceil(&instr.args),
                OpCode::F64Floor => self.f64_floor(&instr.args),
                OpCode::F64Trunc => self.f64_trunc(&instr.args),
                OpCode::F64Nearest => self.f64_nearest(&instr.args),
                OpCode::F64Sqrt => self.f64_sqrt(&instr.args),
                OpCode::I32Add => self.i32_add(&instr.args),
                OpCode::I32Sub => self.i32_sub(&instr.args),
                OpCode::I32Mul => self.i32_mul(&instr.args),
                OpCode::I32DivS => self.i32_divs(&instr.args),
                OpCode::I32DivU => self.i32_divu(&instr.args),
                OpCode::I32RemS => self.i32_rems(&instr.args),
                OpCode::I32RemU => self.i32_remu(&instr.args),
                OpCode::I32And => self.i32_and(&instr.args),
                OpCode::I32Or => self.i32_or(&instr.args),
                OpCode::I32Xor => self.i32_xor(&instr.args),
                OpCode::I32Shl => self.i32_shl(&instr.args),
                OpCode::I32ShrS => self.i32_shrs(&instr.args),
                OpCode::I32ShrU => self.i32_shru(&instr.args),
                OpCode::I32Rotl => self.i32_rotl(&instr.args),
                OpCode::I32Rotr => self.i32_rotr(&instr.args),
                OpCode::I64Add => self.i64_add(&instr.args),
                OpCode::I64Sub => self.i64_sub(&instr.args),
                OpCode::I64Mul => self.i64_mul(&instr.args),
                OpCode::I64DivS => self.i64_divs(&instr.args),
                OpCode::I64DivU => self.i64_divu(&instr.args),
                OpCode::I64RemS => self.i64_rems(&instr.args),
                OpCode::I64RemU => self.i64_remu(&instr.args),
                OpCode::I64And => self.i64_and(&instr.args),
                OpCode::I64Or => self.i64_or(&instr.args),
                OpCode::I64Xor => self.i64_xor(&instr.args),
                OpCode::I64Shl => self.i64_shl(&instr.args),
                OpCode::I64ShrS => self.i64_shrs(&instr.args),
                OpCode::I64ShrU => self.i64_shru(&instr.args),
                OpCode::I64Rotl => self.i64_rotl(&instr.args),
                OpCode::I64Rotr => self.i64_rotr(&instr.args),
                OpCode::F32Add => self.f32_add(&instr.args),
                OpCode::F32Sub => self.f32_sub(&instr.args),
                OpCode::F32Mul => self.f32_mul(&instr.args),
                OpCode::F32Div => self.f32_div(&instr.args),
                OpCode::F32Min => self.f32_min(&instr.args),
                OpCode::F32Max => self.f32_max(&instr.args),
                OpCode::F32CopySign => self.f32_copy_sign(&instr.args),
                OpCode::F64Add => self.f64_add(&instr.args),
                OpCode::F64Sub => self.f64_sub(&instr.args),
                OpCode::F64Mul => self.f64_mul(&instr.args),
                OpCode::F64Div => self.f64_div(&instr.args),
                OpCode::F64Min => self.f64_min(&instr.args),
                OpCode::F64Max => self.f64_max(&instr.args),
                OpCode::F64CopySign => self.f64_copy_sign(&instr.args),
                OpCode::I32WrapI64 => self.i32_wrap_i64(&instr.args),
                OpCode::I64ExtendI32S => self.i64_extend_i32(&instr.args),
                OpCode::I64ExtendI32U => self.i64_extend_u32(&instr.args),
                OpCode::I32Extend8S => self.i32_extend_8(&instr.args),
                OpCode::I32Extend16S => self.i32_extend_16(&instr.args),
                OpCode::I64Extend8S => self.i64_extend_8(&instr.args),
                OpCode::I64Extend16S => self.i64_extend_16(&instr.args),
                OpCode::I64Extend32S => self.i64_extend_32(&instr.args),
                OpCode::I32TruncF32S => self.i32_trunc_f32(&instr.args),
                OpCode::I32TruncF32U => self.u32_trunc_f32(&instr.args),
                OpCode::I32TruncF64S => self.i32_trunc_f64(&instr.args),
                OpCode::I32TruncF64U => self.u32_trunc_f64(&instr.args),
                OpCode::I64TruncF32S => self.i64_trunc_f32(&instr.args),
                OpCode::I64TruncF32U => self.u64_trunc_f32(&instr.args),
                OpCode::I64TruncF64S => self.i64_trunc_f64(&instr.args),
                OpCode::I64TruncF64U => self.u64_trunc_f64(&instr.args),
                OpCode::F32ConvertI32S => self.f32_convert_i32(&instr.args),
                OpCode::F32ConvertI32U => self.f32_convert_u32(&instr.args),
                OpCode::F32ConvertI64S => self.f32_convert_i64(&instr.args),
                OpCode::F32ConvertI64U => self.f32_convert_u64(&instr.args),
                OpCode::F64ConvertI32S => self.f64_convert_i32(&instr.args),
                OpCode::F64ConvertI32U => self.f64_convert_u32(&instr.args),
                OpCode::F64ConvertI64S => self.f64_convert_i64(&instr.args),
                OpCode::F64ConvertI64U => self.f64_convert_u64(&instr.args),
                OpCode::F32DemoteF64 => self.f32_demote_f64(&instr.args),
                OpCode::F64PromoteF32 => self.f64_promote_f32(&instr.args),
                OpCode::I32ReinterpretF32 => {
                    self.i32_reinterpret_f32(&instr.args)
                }
                OpCode::I64ReinterpretF64 => {
                    self.i64_reinterpret_f64(&instr.args)
                }
                OpCode::F32ReinterpretI32 => {
                    self.f32_reinterpret_i32(&instr.args)
                }
                OpCode::F64ReinterpretI64 => {
                    self.f64_reinterpret_i64(&instr.args)
                }
                _ => {}
            }
        }

        // dummy call
        fn call(&self, args: &Option<Box<dyn Any>>) {
            let idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let name = &self.module.import_sec[*idx as usize].member_name;
            let mut stack = self.operand_stack.borrow_mut();

            match name.as_str() {
                "assert_true" => assert_eq!(stack.pop_bool(), true),
                "assert_false" => assert_eq!(stack.pop_bool(), false),
                "assert_eq_i32" => assert_eq!(stack.pop_u32(), stack.pop_u32()),
                "assert_eq_i64" => assert_eq!(stack.pop_u64(), stack.pop_u64()),
                "assert_eq_f32" => assert_eq!(stack.pop_f32(), stack.pop_f32()),
                "assert_eq_f64" => assert_eq!(stack.pop_f64(), stack.pop_f64()),
                _ => {}
            }
        }

        // 参数指令实现
        fn drop_value(&self, _arg: &Option<Box<dyn Any>>) {
            self.operand_stack.borrow_mut().pop_u64();
        }

        fn select(&self, _arg: &Option<Box<dyn Any>>) {
            let mut op_stack = self.operand_stack.borrow_mut();
            let v1 = op_stack.pop_bool();
            let v2 = op_stack.pop_u64();
            let v3 = op_stack.pop_u64();
            if v1 {
                op_stack.push_u64(v3);
            } else {
                op_stack.push_u64(v2);
            }
        }

        // 数值指令实现
        // part 1: 常量指令，共4条
        fn i32_const(&self, args: &Option<Box<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<i32>().unwrap();
            self.operand_stack.borrow_mut().push_i32(*arg);
        }

        fn i64_const(&self, args: &Option<Box<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<i64>().unwrap();
            self.operand_stack.borrow_mut().push_i64(*arg);
        }

        fn f32_const(&self, args: &Option<Box<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<f32>().unwrap();
            self.operand_stack.borrow_mut().push_f32(*arg);
        }

        fn f64_const(&self, args: &Option<Box<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<f64>().unwrap();
            self.operand_stack.borrow_mut().push_f64(*arg);
        }

        // part2: 测试指令
        fn i32_eqz(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let value = stack.pop_i32();
            stack.push_bool(value == 0);
        }

        fn i64_eqz(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let value = stack.pop_i64();
            stack.push_bool(value == 0);
        }

        // part2: 比较指令，共32条
        // i32 相关
        fn i32_eq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_bool(v1 == v2);
        }

        fn i32_neq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_bool(v1 != v2);
        }

        fn i32_lts(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_bool(v1 < v2);
        }

        fn i32_ltu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_bool(v1 < v2);
        }

        fn i32_gts(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_bool(v1 > v2);
        }

        fn i32_gtu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_bool(v1 > v2);
        }

        fn i32_les(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_bool(v1 <= v2);
        }

        fn i32_leu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_bool(v1 <= v2);
        }

        fn i32_ges(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_bool(v1 >= v2);
        }

        fn i32_geu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_bool(v1 >= v2);
        }

        // i64 相关
        fn i64_eq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_bool(v1 == v2);
        }

        fn i64_neq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_bool(v1 != v2);
        }

        fn i64_lts(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_bool(v1 < v2);
        }

        fn i64_ltu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_bool(v1 < v2);
        }

        fn i64_gts(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_bool(v1 > v2);
        }

        fn i64_gtu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_bool(v1 > v2);
        }

        fn i64_les(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_bool(v1 <= v2);
        }

        fn i64_leu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_bool(v1 <= v2);
        }

        fn i64_ges(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_bool(v1 >= v2);
        }

        fn i64_geu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_bool(v1 >= v2);
        }

        // f32 相关
        fn f32_eq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_bool(v1 == v2);
        }

        fn f32_neq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_bool(v1 != v2);
        }

        fn f32_lt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_bool(v1 < v2);
        }

        fn f32_gt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_bool(v1 > v2);
        }

        fn f32_le(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_bool(v1 <= v2);
        }

        fn f32_ge(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_bool(v1 >= v2);
        }

        // f64 相关
        fn f64_eq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_bool(v1 == v2);
        }

        fn f64_neq(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_bool(v1 != v2);
        }

        fn f64_lt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_bool(v1 < v2);
        }

        fn f64_gt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_bool(v1 > v2);
        }

        fn f64_le(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_bool(v1 <= v2);
        }

        fn f64_ge(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_bool(v1 >= v2);
        }

        // 一元算术指令，共6条
        fn i32_clz(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_u32();
            stack.push_u32(val.leading_zeros());
        }

        fn i32_ctz(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_u32();
            stack.push_u32(val.trailing_zeros());
        }

        fn i32_pop_cnt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_u32();
            stack.push_u32(val.count_ones());
        }

        fn i64_clz(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_u64();
            stack.push_u32(val.leading_zeros());
        }

        fn i64_ctz(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_u64();
            stack.push_u32(val.trailing_zeros());
        }

        fn i64_pop_cnt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_u64();
            stack.push_u32(val.count_ones());
        }

        fn f32_abs(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f32();
            stack.push_f32(val.abs());
        }

        fn f32_neg(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f32();
            stack.push_f32(-val);
        }

        fn f32_ceil(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f32();
            stack.push_f32(val.ceil());
        }

        fn f32_floor(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f32();
            stack.push_f32(val.floor());
        }

        fn f32_trunc(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f32();
            stack.push_f32(val.trunc());
        }

        fn f32_nearest(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f32();
            stack.push_f32(val.round());
        }

        fn f32_sqrt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f32();
            stack.push_f32(val.sqrt());
        }

        fn f64_abs(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f64();
            stack.push_f64(val.abs());
        }

        fn f64_neg(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f64();
            stack.push_f64(-val);
        }

        fn f64_ceil(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f64();
            stack.push_f64(val.ceil());
        }

        fn f64_floor(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f64();
            stack.push_f64(val.floor());
        }

        fn f64_trunc(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f64();
            stack.push_f64(val.trunc());
        }

        fn f64_nearest(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f64();
            stack.push_f64(val.round());
        }

        fn f64_sqrt(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let val = stack.pop_f64();
            stack.push_f64(val.sqrt());
        }

        // 二元算术指令
        // part1: 整形算术运算，共30条
        fn i32_add(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 + v2);
        }

        fn i32_sub(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 - v2);
        }

        fn i32_mul(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 * v2);
        }

        fn i32_divs(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 / v2);
        }

        fn i32_divu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_u32(v1 / v2);
        }

        fn i32_rems(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 % v2);
        }

        fn i32_remu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_u32(v1 % v2);
        }

        fn i32_and(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 & v2);
        }

        fn i32_or(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 | v2);
        }

        fn i32_xor(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 ^ v2);
        }

        fn i32_shl(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 << (v2 % 64));
        }

        fn i32_shrs(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1 >> (v2 % 64));
        }

        fn i32_shru(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u32();
            let v1 = stack.pop_u32();
            stack.push_u32(v1 >> (v2 % 64));
        }

        fn i32_rotl(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1.rotate_left(v2 as u32));
        }

        fn i32_rotr(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i32();
            let v1 = stack.pop_i32();
            stack.push_i32(v1.rotate_right(v2 as u32));
        }

        fn i64_add(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 + v2);
        }

        fn i64_sub(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 - v2);
        }

        fn i64_mul(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 * v2);
        }

        fn i64_divs(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 / v2);
        }

        fn i64_divu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_u64(v1 / v2);
        }

        fn i64_rems(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 % v2);
        }

        fn i64_remu(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_u64(v1 % v2);
        }

        fn i64_and(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 & v2);
        }

        fn i64_or(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 | v2);
        }

        fn i64_xor(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 ^ v2);
        }

        fn i64_shl(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 << (v2 % 64));
        }

        fn i64_shrs(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1 >> (v2 % 64));
        }

        fn i64_shru(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_u64();
            let v1 = stack.pop_u64();
            stack.push_u64(v1 >> (v2 % 64));
        }

        fn i64_rotl(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1.rotate_left(v2 as u32));
        }

        fn i64_rotr(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_i64();
            let v1 = stack.pop_i64();
            stack.push_i64(v1.rotate_right(v2 as u32));
        }

        // part2: 浮点算术运算，共14条
        fn f32_add(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_f32(v1 + v2);
        }

        fn f32_sub(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_f32(v1 - v2);
        }

        fn f32_mul(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_f32(v1 * v2);
        }

        fn f32_div(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_f32(v1 / v2);
        }

        fn f32_min(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_f32(v1.min(v2));
        }

        fn f32_max(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_f32(v1.max(v2));
        }

        fn f32_copy_sign(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f32();
            let v1 = stack.pop_f32();
            stack.push_f32(v1.copysign(v2));
        }

        fn f64_add(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_f64(v1 + v2);
        }

        fn f64_sub(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_f64(v1 - v2);
        }

        fn f64_mul(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_f64(v1 * v2);
        }

        fn f64_div(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_f64(v1 / v2);
        }

        fn f64_min(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_f64(v1.min(v2));
        }

        fn f64_max(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_f64(v1.max(v2));
        }

        fn f64_copy_sign(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v2 = stack.pop_f64();
            let v1 = stack.pop_f64();
            stack.push_f64(v1.copysign(v2));
        }

        // 类型转换指令
        // part1: 整数截断，共1条指令
        fn i32_wrap_i64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_u64();
            stack.push_u32(v as u32);
        }
        // part2: 整数拉升，共7条指令
        fn i64_extend_i32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i32();
            stack.push_u64(v as u64);
        }

        fn i64_extend_u32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_u32();
            stack.push_u64(v as u64);
        }

        fn i32_extend_8(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i32() as i8;
            stack.push_i32(v as i32);
        }

        fn i32_extend_16(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i32() as i16;
            stack.push_i32(v as i32);
        }

        fn i64_extend_8(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i64() as i8;
            stack.push_i64(v as i64);
        }

        fn i64_extend_16(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i64() as i16;
            stack.push_i64(v as i64);
        }

        fn i64_extend_32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i64() as i32;
            stack.push_i64(v as i64);
        }
        // part3: 浮点数截断，共9条指令
        fn i32_trunc_f32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f32();
            stack.push_i32(v.trunc() as i32);
        }

        fn u32_trunc_f32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f32();
            stack.push_u32(v.trunc() as u32);
        }

        fn i32_trunc_f64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f64();
            stack.push_i32(v.trunc() as i32);
        }

        fn u32_trunc_f64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f64();
            stack.push_u32(v.trunc() as u32);
        }

        fn i64_trunc_f32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f32();
            stack.push_i64(v.trunc() as i64);
        }

        fn u64_trunc_f32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f32();
            stack.push_u64(v.trunc() as u64);
        }

        fn i64_trunc_f64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f64();
            stack.push_i64(v.trunc() as i64);
        }

        fn u64_trunc_f64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f64();
            stack.push_u64(v.trunc() as u64);
        }

        // part4: 整数转换，共8条指令
        fn f32_convert_i32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i32();
            stack.push_f32(v as f32);
        }

        fn f32_convert_u32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_u32();
            stack.push_f32(v as f32);
        }

        fn f32_convert_i64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i64();
            stack.push_f32(v as f32);
        }

        fn f32_convert_u64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_u64();
            stack.push_f32(v as f32);
        }

        fn f64_convert_i32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i32();
            stack.push_f64(v as f64);
        }

        fn f64_convert_u32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_u32();
            stack.push_f64(v as f64);
        }

        fn f64_convert_i64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_i64();
            stack.push_f64(v as f64);
        }

        fn f64_convert_u64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_u64();
            stack.push_f64(v as f64);
        }
        // part5: 浮点数精度调整，共2条指令
        fn f32_demote_f64(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f64();
            stack.push_f32(v as f32);
        }

        fn f64_promote_f32(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            let v = stack.pop_f32();
            stack.push_f64(v as f64);
        }
        // part6: 比特位重新解释，共4条指令，只需重新解释类型，无需做任何操作
        fn i32_reinterpret_f32(&self, _args: &Option<Box<dyn Any>>) {}
        fn i64_reinterpret_f64(&self, _args: &Option<Box<dyn Any>>) {}
        fn f32_reinterpret_i32(&self, _args: &Option<Box<dyn Any>>) {}
        fn f64_reinterpret_i64(&self, _args: &Option<Box<dyn Any>>) {}

        // 内存相关指令
        // helper function
        fn get_offset(&self, args: &Option<Box<dyn Any>>) -> usize {
            let arg = args.as_ref().unwrap().downcast_ref::<MemArg>().unwrap(); 
            let mut stack = self.operand_stack.borrow_mut();
            // 动态的操作数偏移量 + 静态的立即数偏移量，结果可能溢出u32，得用u64表示
            (stack.pop_u32() + arg.offset) as usize
        }

        fn read_u8(&self, args: &Option<Box<dyn Any>>) -> u8 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8];
            self.memory.read(offset, &mut buf[..]);
            buf[0]
        }

        fn read_u16(&self, args: &Option<Box<dyn Any>>) -> u16 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8;2];
            self.memory.read(offset, &mut buf[..]);
            u16::from_le_bytes(buf.try_into().unwrap())
        }

        fn read_u32(&self, args: &Option<Box<dyn Any>>) -> u32 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8;4];
            self.memory.read(offset, &mut buf[..]);
            u32::from_le_bytes(buf.try_into().unwrap())
        }

        fn read_u64(&self, args: &Option<Box<dyn Any>>) -> u64 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8;8];
            self.memory.read(offset, &mut buf[..]);
            u64::from_le_bytes(buf.try_into().unwrap())
        }

        // part1: size 和 grow
        fn memory_size(&self, _args: &Option<Box<dyn Any>>) {
            let mut stack = self.operand_stack.borrow_mut();
            stack.push_u32(self.memory.size() as u32);
        }

        fn memory_grow(&self, _args: &Option<Box<dyn Any>>) {
           let mut stack = self.operand_stack.borrow_mut();
           let grow_size = stack.pop_u32();
           let old_size = self.memory.grow(grow_size as usize);
           stack.push_u32(old_size as u32);
        }

        // part2: load
        fn i64_load(&self, args: &Option<Box<dyn Any>>) {
            let val = self.read_u64(args);
            let mut stack = self.operand_stack.borrow_mut();
            stack.push_u64(val);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_operand_stack() {
            let mut stack = OperandStack::new();
            stack.push_bool(true);
            stack.push_bool(false);
            stack.push_u32(1);
            stack.push_i32(-2);
            stack.push_u64(3);
            stack.push_i64(-4);
            stack.push_f32(5.5);
            stack.push_f64(6.5);

            assert_eq!(stack.pop_f64(), 6.5);
            assert_eq!(stack.pop_f32(), 5.5);
            assert_eq!(stack.pop_i64(), -4);
            assert_eq!(stack.pop_u64(), 3);
            assert_eq!(stack.pop_i32(), -2);
            assert_eq!(stack.pop_u32(), 1);
            assert_eq!(stack.pop_bool(), false);
            assert_eq!(stack.pop_bool(), true);
            assert_eq!(stack.length(), 0);
        }
    }
}
