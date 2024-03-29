pub mod interpreter {
    use std::{any::Any, rc::Rc, vec};

    use crate::module::{instruction::instruction::BrArgs, *};

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

        fn get_operand(&self, idx: usize) -> u64 {
            self.slots[idx]
        }

        fn set_operand(&mut self, idx: usize, val: u64) {
            self.slots[idx] = val;
        }

        fn push_u64s(&mut self, vals: &mut Vec<u64>) {
            self.slots.append(vals)
        }

        fn pop_u64s(&mut self, n: usize) -> Vec<u64> {
            let ret = self.slots[(self.slots.len() - n)..].to_vec();
            self.slots.drain(self.slots.len() - n..);
            ret
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

    struct ControlFrame {
        opcode: OpCode,
        block_type: FuncType,
        instrs: Vec<Instruction>,
        bp: usize, // base pointer
        pc: i32,   // program counter
    }

    impl ControlFrame {
        fn new(
            opcode: OpCode,
            block_type: FuncType,
            instrs: Vec<Instruction>,
            bp: usize,
        ) -> ControlFrame {
            ControlFrame {
                opcode,
                block_type,
                instrs,
                bp,
                pc: 0,
            }
        }
    }

    struct ControlStack {
        frames: Vec<ControlFrame>,
    }

    impl ControlStack {
        fn new() -> ControlStack {
            ControlStack { frames: vec![] }
        }

        fn push_control_frame(&mut self, cf: ControlFrame) {
            self.frames.push(cf)
        }

        fn pop_control_frame(&mut self) -> ControlFrame {
            self.frames.pop().unwrap()
        }

        fn control_depth(&self) -> usize {
            self.frames.len()
        }

        fn top_control_frame(&mut self) -> &mut ControlFrame {
            let idx = self.frames.len() - 1;
            &mut self.frames[idx]
        }

        fn top_call_frame(&self) -> (Option<&ControlFrame>, usize) {
            for (idx, cf) in self.frames.iter().rev().enumerate() {
                if cf.opcode == OpCode::Call {
                    return (Some(cf), idx);
                }
            }
            return (None, usize::MAX);
        }
    }

    struct Memory {
        mem_type: MemType,
        data: Vec<u8>,
    }

    impl Memory {
        fn new(mem_type: MemType) -> Memory {
            let min_page_size = mem_type.min;
            Memory {
                mem_type,
                data: vec![0; (min_page_size * PAGE_SIZE) as usize],
            }
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
            self.data.extend(vec![0; n * PAGE_SIZE].iter());
            old_size
        }

        fn read(&mut self, offset: usize, buf: &mut [u8]) {
            self.check_offset(offset, buf.len());
            buf.copy_from_slice(&self.data[offset..offset + buf.len()]);
        }

        fn write(&mut self, offset: usize, data: &[u8]) {
            self.check_offset(offset, data.len());
            self.data[offset..offset + data.len()].copy_from_slice(data);
        }

        fn check_offset(&mut self, offset: usize, length: usize) {
            if self.data.len() - length < offset {
                panic!("Memory out of bounds");
            }
        }
    }

    struct GlobalVar {
        global_type: GlobalType,
        val: u64,
    }

    impl GlobalVar {
        fn new(global_type: GlobalType, val: u64) -> GlobalVar {
            GlobalVar { global_type, val }
        }

        fn get_as_u64(&self) -> u64 {
            self.val
        }

        fn set_as_u64(&mut self, val: u64) {
            if !self.global_type.mutable {
                panic!("Immutable global!");
            }
            self.val = val;
        }
    }

    type WasmVal = Box<dyn Any>;
    type NativeFunc = fn(Vec<WasmVal>) -> Vec<WasmVal>;

    #[derive(Clone, Default)]
    struct VMFunc {
        func_type: FuncType,
        code: Option<Code>,
        native_func: Option<NativeFunc>,
    }

    impl VMFunc {
        fn new_internal_func(func_type: FuncType, code: Code) -> VMFunc {
            VMFunc {
                func_type,
                code: Some(code),
                native_func: None,
            }
        }

        fn new_external_func(
            func_type: FuncType,
            native_func: NativeFunc,
        ) -> VMFunc {
            VMFunc {
                func_type,
                code: None,
                native_func: Some(native_func),
            }
        }
    }

    struct Table {
        elem_type: TableType,
        elems: Vec<VMFunc>,
    }

    impl Table {
        fn new(elem_type: TableType) -> Table {
            let min = elem_type.limits.min;
            Table {
                elem_type,
                elems: vec![VMFunc::default(); min],
            }
        }

        fn get_type(&self) -> TableType {
            self.elem_type
        }

        fn size(&self) -> usize {
            self.elems.len()
        }

        fn grow(&mut self, n: usize) {
            let m = vec![VMFunc::default(); n];
            self.elems.extend(m);
        }

        fn get_elem(&self, idx: usize) -> VMFunc {
            self.elems[idx].clone()
        }

        fn set_elem(&mut self, idx: usize, elem: VMFunc) {
            self.elems[idx] = elem;
        }
    }

    pub struct VM<'a> {
        operand_stack: OperandStack,
        module: &'a Module,
        memory: Memory,
        control_stack: ControlStack,
        local_0_idx: usize,
        globals: Vec<GlobalVar>,
        vm_funcs: Vec<VMFunc>,
        table: Option<Table>,
    }

    impl<'a> VM<'a> {
        fn new(module: &Module) -> VM {
            let memory: Memory;
            if module.mem_sec.len() > 0 {
                memory = Memory::new(module.mem_sec[0]);
            } else {
                memory = Memory::new(MemType { min: 0, max: None });
            }
            let operand_stack = OperandStack::new();
            VM {
                operand_stack,
                module,
                memory,
                local_0_idx: usize::MAX,
                globals: vec![],
                control_stack: ControlStack::new(),
                vm_funcs: vec![],
                table: None,
            }
        }

        fn init_table(&mut self) {
            if self.module.table_sec.len() > 0 {
                self.table = Some(Table::new(self.module.table_sec[0]));
                for elem in &self.module.elem_sec {
                    for instr in &elem.offset {
                        self.exec_instr(instr);
                    }
                    let offset = self.operand_stack.pop_u32();
                    for (idx, func_idx) in elem.init.iter().enumerate() {
                        self.table.as_mut().unwrap().set_elem(
                            offset as usize + idx,
                            self.vm_funcs[*func_idx as usize].clone(),
                        );
                    }
                }
            }
        }

        fn init_memory(&mut self) {
            for data in &self.module.data_sec {
                for instr in &data.offset {
                    self.exec_instr(instr);
                }
                self.memory.write(
                    self.operand_stack.pop_u64() as usize,
                    &data.init[..],
                );
            }
        }

        fn init_globals(&mut self) {
            for global in &self.module.global_sec {
                for instr in &global.init_expr {
                    self.exec_instr(instr);
                }
                self.globals.push(GlobalVar::new(
                    global.global_type,
                    self.operand_stack.pop_u64(),
                ));
            }
        }

        fn get_main_idx(&self) -> Option<u32> {
            for exp in &self.module.export_sec {
                match exp.desc {
                    ExportDesc::Func(idx) if exp.name == "main" => {
                        return Some(idx)
                    }
                    _ => {}
                }
            }
            None
        }

        pub fn exec_main(module: &Module) {
            let mut vm = VM::new(module);
            vm.init_memory();
            vm.init_globals();
            vm.init_funcs();
            vm.init_table();
            if let Some(start_sec_id) = module.start_sec {
                vm.call(&Some(Rc::new(start_sec_id)));
            } else {
                if let Some(idx) = vm.get_main_idx() {
                    vm.call(&Some(Rc::new(idx)));
                } else {
                    panic!("No start sec!");
                }
            }
            vm.main_loop();
        }

        fn main_loop(&mut self) {
            let depth = self.control_stack.control_depth();
            // 执行栈帧中的每条指令
            while self.control_stack.control_depth() >= depth {
                let cf = self.control_stack.top_control_frame();
                if cf.pc as usize == cf.instrs.len() {
                    self.exit_block(); // 已经执行完了一个control frame
                } else {
                    let instr = cf.instrs[cf.pc as usize].clone();
                    cf.pc += 1;
                    self.exec_instr(&instr);
                }
            }
        }

        fn enter_block(
            &mut self,
            opcode: OpCode,
            bt: FuncType,
            instrs: Vec<Instruction>,
        ) {
            // enter_block 时参数已在栈顶(调用方将参数入栈)
            let bp = self.operand_stack.length() - bt.params_types.len();
            let cf = ControlFrame::new(opcode, bt, instrs, bp);
            self.control_stack.push_control_frame(cf);
            if opcode == OpCode::Call {
                self.local_0_idx = bp;
            }
        }

        fn exit_block(&mut self) {
            let cf = self.control_stack.pop_control_frame();
            self.clear_block(cf);
        }

        fn clear_block(&mut self, cf: ControlFrame) {
            // 弹出结果，结果已在栈顶
            let mut results = self
                .operand_stack
                .pop_u64s(cf.block_type.result_types.len());
            // 清除其他变量（比如局部变量和参数）
            self.operand_stack
                .pop_u64s(self.operand_stack.length() - cf.bp);
            // 将结果放回到栈顶
            self.operand_stack.push_u64s(&mut results);
            if cf.opcode == OpCode::Call
                && self.control_stack.control_depth() > 0
            {
                // 如果是函数调用的退出，还需要恢复 local_0_idx
                let (last_call_frame, _) = self.control_stack.top_call_frame();
                self.local_0_idx = last_call_frame.unwrap().bp;
            }
        }

        fn _reset_block(&mut self, cf: &ControlFrame) {
            let mut results = self
                .operand_stack
                .pop_u64s(cf.block_type.params_types.len());
            self.operand_stack
                .pop_u64s(self.operand_stack.length() - cf.bp);
            self.operand_stack.push_u64s(&mut results);
        }

        fn exec_instr(&mut self, instr: &Instruction) {
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
                OpCode::MemorySize => self.memory_size(&instr.args),
                OpCode::MemoryGrow => self.memory_grow(&instr.args),
                OpCode::I32Load => self.i32_load(&instr.args),
                OpCode::I64Load => self.i64_load(&instr.args),
                OpCode::F32Load => self.f32_load(&instr.args),
                OpCode::F64Load => self.f64_load(&instr.args),
                OpCode::I32Load8S => self.i32_load_8s(&instr.args),
                OpCode::I32Load8U => self.i32_load_8u(&instr.args),
                OpCode::I32Load16S => self.i32_load_16s(&instr.args),
                OpCode::I32Load16U => self.i32_load_16u(&instr.args),
                OpCode::I64Load8S => self.i64_load_8s(&instr.args),
                OpCode::I64Load8U => self.i64_load_8u(&instr.args),
                OpCode::I64Load16S => self.i64_load_16s(&instr.args),
                OpCode::I64Load16U => self.i64_load_16u(&instr.args),
                OpCode::I64Load32S => self.i64_load_32s(&instr.args),
                OpCode::I64Load32U => self.i64_load_32u(&instr.args),
                OpCode::I32Store => self.i32_store(&instr.args),
                OpCode::I64Store => self.i64_store(&instr.args),
                OpCode::F32Store => self.f32_store(&instr.args),
                OpCode::F64Store => self.f64_store(&instr.args),
                OpCode::I32Store8 => self.i32_store_8(&instr.args),
                OpCode::I32Store16 => self.i32_store_16(&instr.args),
                OpCode::I64Store8 => self.i64_store_8(&instr.args),
                OpCode::I64Store16 => self.i64_store_16(&instr.args),
                OpCode::I64Store32 => self.i64_store_32(&instr.args),
                OpCode::LocalGet => self.local_get(&instr.args),
                OpCode::LocalSet => self.local_set(&instr.args),
                OpCode::LocalTee => self.local_tee(&instr.args),
                OpCode::GlobalGet => self.global_get(&instr.args),
                OpCode::GlobalSet => self.global_set(&instr.args),
                OpCode::Br => self.br(&instr.args),
                OpCode::BrTable => self.br_table(&instr.args),
                OpCode::BrIf => self.br_if(&instr.args),
                OpCode::Block => self.block(&instr.args),
                OpCode::Loop => self.loop_instr(&instr.args),
                OpCode::If => self.if_instr(&instr.args),
                OpCode::Return => self.return_instr(&instr.args),
                OpCode::CallIndirect => self.call_indrect(&instr.args),
                OpCode::Unreachable => self.unreachable(&instr.args),
                OpCode::Nop => self.nop(&instr.args),
                _ => {}
            }
        }

        fn init_funcs(&mut self) {
            self.link_native_funcs();
            for (idx, func_idx) in self.module.func_sec.iter().enumerate() {
                self.vm_funcs.push(VMFunc::new_internal_func(
                    self.module.type_sec[*func_idx as usize].clone(),
                    self.module.code_sec[idx].clone(),
                ));
            }
        }

        fn print_char(args: Vec<WasmVal>) -> Vec<WasmVal> {
            assert!(args.len() == 1);
            let arg = args[0].downcast_ref::<i32>().unwrap();
            print!("{}", *arg as u8 as char);
            vec![]
        }

        fn assert_true(args: Vec<WasmVal>) -> Vec<WasmVal> {
            assert!(args.len() == 1);
            let arg = args[0].downcast_ref::<bool>().unwrap();
            assert!(arg);
            vec![]
        }

        fn assert_false(args: Vec<WasmVal>) -> Vec<WasmVal> {
            assert!(args.len() == 1);
            let arg = args[0].downcast_ref::<bool>().unwrap();
            assert!(!arg);
            vec![]
        }

        fn assert_eq_i32(args: Vec<WasmVal>) -> Vec<WasmVal> {
            assert!(args.len() == 2);
            let left = args[0].downcast_ref::<i32>().unwrap();
            let right = args[1].downcast_ref::<i32>().unwrap();
            assert_eq!(left, right);
            vec![]
        }

        fn assert_eq_i64(args: Vec<WasmVal>) -> Vec<WasmVal> {
            assert!(args.len() == 2);
            let left = args[0].downcast_ref::<i64>().unwrap();
            let right = args[1].downcast_ref::<i64>().unwrap();
            assert_eq!(left, right);
            vec![]
        }

        fn assert_eq_f32(args: Vec<WasmVal>) -> Vec<WasmVal> {
            assert!(args.len() == 2);
            let left = args[0].downcast_ref::<f32>().unwrap();
            let right = args[1].downcast_ref::<f32>().unwrap();
            assert_eq!(left, right);
            vec![]
        }

        fn assert_eq_f64(args: Vec<WasmVal>) -> Vec<WasmVal> {
            assert!(args.len() == 2);
            let left = args[0].downcast_ref::<f64>().unwrap();
            let right = args[1].downcast_ref::<f64>().unwrap();
            assert_eq!(left, right);
            vec![]
        }

        fn link_native_funcs(&mut self) {
            for imp in &self.module.import_sec {
                if imp.module_name == "env" {
                    match imp.desc {
                        ImportDesc::Func(func_idx) => {
                            let ft =
                                self.module.type_sec[func_idx as usize].clone();
                            match imp.member_name.as_str() {
                                "print_char" => {
                                    self.vm_funcs.push(
                                        VMFunc::new_external_func(
                                            ft,
                                            VM::print_char,
                                        ),
                                    );
                                }
                                "assert_true" => {
                                    self.vm_funcs.push(
                                        VMFunc::new_external_func(
                                            ft,
                                            VM::assert_true,
                                        ),
                                    );
                                }
                                "assert_false" => {
                                    self.vm_funcs.push(
                                        VMFunc::new_external_func(
                                            ft,
                                            VM::assert_false,
                                        ),
                                    );
                                }
                                "assert_eq_i32" => {
                                    self.vm_funcs.push(
                                        VMFunc::new_external_func(
                                            ft,
                                            VM::assert_eq_i32,
                                        ),
                                    );
                                }
                                "assert_eq_i64" => {
                                    self.vm_funcs.push(
                                        VMFunc::new_external_func(
                                            ft,
                                            VM::assert_eq_i64,
                                        ),
                                    );
                                }
                                "assert_eq_f32" => {
                                    self.vm_funcs.push(
                                        VMFunc::new_external_func(
                                            ft,
                                            VM::assert_eq_f32,
                                        ),
                                    );
                                }
                                "assert_eq_f64" => {
                                    self.vm_funcs.push(
                                        VMFunc::new_external_func(
                                            ft,
                                            VM::assert_eq_f64,
                                        ),
                                    );
                                }
                                _ => {
                                    panic!("Should not reach here.");
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        fn call_internal_func(&mut self, func: &VMFunc) {
            self.enter_block(
                OpCode::Call,
                func.func_type.clone(),
                func.code.clone().unwrap().expr,
            );
            // alloc locals
            let local_cnt = func.code.as_ref().unwrap().get_local_count();
            for _ in 0..local_cnt {
                self.operand_stack.push_u64(0);
            }
        }

        fn call_external_func(&mut self, f: &VMFunc) {
            let args = self.pop_args(&f.func_type);
            let results = f.native_func.unwrap()(args);
            self.push_results(&f.func_type, results);
        }

        fn pop_args(&mut self, ft: &FuncType) -> Vec<Box<dyn Any>> {
            let mut args = Vec::with_capacity(ft.params_types.len());
            for i in 0..ft.params_types.len() {
                let val = self.operand_stack.pop_u64();
                args.push(self.wrap_u64(&ft.params_types[i], val));
            }
            args.into_iter().rev().collect()
        }

        fn push_results(&mut self, ft: &FuncType, results: Vec<Box<dyn Any>>) {
            for result in results {
                let val = self.unwrap_u64(&ft.result_types[0], result);
                self.operand_stack.push_u64(val);
            }
        }

        fn wrap_u64(&mut self, vt: &ValType, val: u64) -> Box<dyn Any> {
            match vt {
                ValType::I32 => Box::new(val as i32),
                ValType::I64 => Box::new(val as i64),
                ValType::F32 => {
                    Box::new(f32::from_le_bytes((val as u32).to_le_bytes()))
                }
                ValType::F64 => Box::new(f64::from_le_bytes(val.to_le_bytes())),
                ValType::FuncRef => panic!("Unreachable."),
            }
        }

        fn unwrap_u64(&mut self, vt: &ValType, val: Box<dyn Any>) -> u64 {
            let val_ref = val.as_ref();
            match vt {
                ValType::I32 => {
                    val_ref.downcast_ref::<i32>().unwrap().to_owned() as u64
                }
                ValType::I64 => {
                    val_ref.downcast_ref::<i64>().unwrap().to_owned() as u64
                }
                ValType::F32 => u64::from_le_bytes(
                    (val_ref.downcast_ref::<f32>().unwrap().to_owned() as f64)
                        .to_le_bytes(),
                ),
                ValType::F64 => u64::from_le_bytes(
                    val_ref.downcast_ref::<f64>().unwrap().to_le_bytes(),
                ),
                ValType::FuncRef => panic!("Unreachable."),
            }
        }

        fn call(&mut self, args: &Option<Rc<dyn Any>>) {
            let idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let f = self.vm_funcs[*idx as usize].clone();
            if f.code.is_some() {
                self.call_internal_func(&f);
            } else if f.native_func.is_some() {
                self.call_external_func(&f);
            }
        }

        // 参数指令实现
        fn drop_value(&mut self, _arg: &Option<Rc<dyn Any>>) {
            self.operand_stack.pop_u64();
        }

        fn select(&mut self, _arg: &Option<Rc<dyn Any>>) {
            let v1 = self.operand_stack.pop_bool();
            let v2 = self.operand_stack.pop_u64();
            let v3 = self.operand_stack.pop_u64();
            if v1 {
                self.operand_stack.push_u64(v3);
            } else {
                self.operand_stack.push_u64(v2);
            }
        }

        // 数值指令实现
        // part 1: 常量指令，共4条
        fn i32_const(&mut self, args: &Option<Rc<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<i32>().unwrap();
            self.operand_stack.push_i32(*arg);
        }

        fn i64_const(&mut self, args: &Option<Rc<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<i64>().unwrap();
            self.operand_stack.push_i64(*arg);
        }

        fn f32_const(&mut self, args: &Option<Rc<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<f32>().unwrap();
            self.operand_stack.push_f32(*arg);
        }

        fn f64_const(&mut self, args: &Option<Rc<dyn Any>>) {
            let arg = args.as_ref().unwrap().downcast_ref::<f64>().unwrap();
            self.operand_stack.push_f64(*arg);
        }

        // part2: 测试指令
        fn i32_eqz(&mut self, _args: &Option<Rc<dyn Any>>) {
            let value = self.operand_stack.pop_i32();
            self.operand_stack.push_bool(value == 0);
        }

        fn i64_eqz(&mut self, _args: &Option<Rc<dyn Any>>) {
            let value = self.operand_stack.pop_i64();
            self.operand_stack.push_bool(value == 0);
        }

        // part2: 比较指令，共32条
        // i32 相关
        fn i32_eq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_bool(v1 == v2);
        }

        fn i32_neq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_bool(v1 != v2);
        }

        fn i32_lts(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_bool(v1 < v2);
        }

        fn i32_ltu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_bool(v1 < v2);
        }

        fn i32_gts(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_bool(v1 > v2);
        }

        fn i32_gtu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_bool(v1 > v2);
        }

        fn i32_les(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_bool(v1 <= v2);
        }

        fn i32_leu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_bool(v1 <= v2);
        }

        fn i32_ges(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_bool(v1 >= v2);
        }

        fn i32_geu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_bool(v1 >= v2);
        }

        // i64 相关
        fn i64_eq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_bool(v1 == v2);
        }

        fn i64_neq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_bool(v1 != v2);
        }

        fn i64_lts(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_bool(v1 < v2);
        }

        fn i64_ltu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_bool(v1 < v2);
        }

        fn i64_gts(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_bool(v1 > v2);
        }

        fn i64_gtu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_bool(v1 > v2);
        }

        fn i64_les(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_bool(v1 <= v2);
        }

        fn i64_leu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_bool(v1 <= v2);
        }

        fn i64_ges(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_bool(v1 >= v2);
        }

        fn i64_geu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_bool(v1 >= v2);
        }

        // f32 相关
        fn f32_eq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_bool(v1 == v2);
        }

        fn f32_neq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_bool(v1 != v2);
        }

        fn f32_lt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_bool(v1 < v2);
        }

        fn f32_gt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_bool(v1 > v2);
        }

        fn f32_le(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_bool(v1 <= v2);
        }

        fn f32_ge(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_bool(v1 >= v2);
        }

        // f64 相关
        fn f64_eq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_bool(v1 == v2);
        }

        fn f64_neq(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_bool(v1 != v2);
        }

        fn f64_lt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_bool(v1 < v2);
        }

        fn f64_gt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_bool(v1 > v2);
        }

        fn f64_le(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_bool(v1 <= v2);
        }

        fn f64_ge(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_bool(v1 >= v2);
        }

        // 一元算术指令，共6条
        fn i32_clz(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u32();
            self.operand_stack.push_u32(val.leading_zeros());
        }

        fn i32_ctz(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u32();
            self.operand_stack.push_u32(val.trailing_zeros());
        }

        fn i32_pop_cnt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u32();
            self.operand_stack.push_u32(val.count_ones());
        }

        fn i64_clz(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.operand_stack.push_u32(val.leading_zeros());
        }

        fn i64_ctz(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.operand_stack.push_u32(val.trailing_zeros());
        }

        fn i64_pop_cnt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.operand_stack.push_u32(val.count_ones());
        }

        fn f32_abs(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(val.abs());
        }

        fn f32_neg(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(-val);
        }

        fn f32_ceil(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(val.ceil());
        }

        fn f32_floor(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(val.floor());
        }

        fn f32_trunc(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(val.trunc());
        }

        fn f32_nearest(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(val.round());
        }

        fn f32_sqrt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(val.sqrt());
        }

        fn f64_abs(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(val.abs());
        }

        fn f64_neg(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(-val);
        }

        fn f64_ceil(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(val.ceil());
        }

        fn f64_floor(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(val.floor());
        }

        fn f64_trunc(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(val.trunc());
        }

        fn f64_nearest(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(val.round());
        }

        fn f64_sqrt(&mut self, _args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(val.sqrt());
        }

        // 二元算术指令
        // part1: 整形算术运算，共30条
        fn i32_add(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 + v2);
        }

        fn i32_sub(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 - v2);
        }

        fn i32_mul(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 * v2);
        }

        fn i32_divs(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 / v2);
        }

        fn i32_divu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_u32(v1 / v2);
        }

        fn i32_rems(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 % v2);
        }

        fn i32_remu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_u32(v1 % v2);
        }

        fn i32_and(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 & v2);
        }

        fn i32_or(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 | v2);
        }

        fn i32_xor(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 ^ v2);
        }

        fn i32_shl(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 << (v2 % 64));
        }

        fn i32_shrs(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1 >> (v2 % 64));
        }

        fn i32_shru(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u32();
            let v1 = self.operand_stack.pop_u32();
            self.operand_stack.push_u32(v1 >> (v2 % 64));
        }

        fn i32_rotl(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1.rotate_left(v2 as u32));
        }

        fn i32_rotr(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i32();
            let v1 = self.operand_stack.pop_i32();
            self.operand_stack.push_i32(v1.rotate_right(v2 as u32));
        }

        fn i64_add(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 + v2);
        }

        fn i64_sub(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 - v2);
        }

        fn i64_mul(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 * v2);
        }

        fn i64_divs(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 / v2);
        }

        fn i64_divu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_u64(v1 / v2);
        }

        fn i64_rems(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 % v2);
        }

        fn i64_remu(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_u64(v1 % v2);
        }

        fn i64_and(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 & v2);
        }

        fn i64_or(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 | v2);
        }

        fn i64_xor(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 ^ v2);
        }

        fn i64_shl(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 << (v2 % 64));
        }

        fn i64_shrs(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1 >> (v2 % 64));
        }

        fn i64_shru(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_u64();
            let v1 = self.operand_stack.pop_u64();
            self.operand_stack.push_u64(v1 >> (v2 % 64));
        }

        fn i64_rotl(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1.rotate_left(v2 as u32));
        }

        fn i64_rotr(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_i64();
            let v1 = self.operand_stack.pop_i64();
            self.operand_stack.push_i64(v1.rotate_right(v2 as u32));
        }

        // part2: 浮点算术运算，共14条
        fn f32_add(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(v1 + v2);
        }

        fn f32_sub(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(v1 - v2);
        }

        fn f32_mul(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(v1 * v2);
        }

        fn f32_div(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(v1 / v2);
        }

        fn f32_min(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(v1.min(v2));
        }

        fn f32_max(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(v1.max(v2));
        }

        fn f32_copy_sign(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f32();
            let v1 = self.operand_stack.pop_f32();
            self.operand_stack.push_f32(v1.copysign(v2));
        }

        fn f64_add(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(v1 + v2);
        }

        fn f64_sub(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(v1 - v2);
        }

        fn f64_mul(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(v1 * v2);
        }

        fn f64_div(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(v1 / v2);
        }

        fn f64_min(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(v1.min(v2));
        }

        fn f64_max(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(v1.max(v2));
        }

        fn f64_copy_sign(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v2 = self.operand_stack.pop_f64();
            let v1 = self.operand_stack.pop_f64();
            self.operand_stack.push_f64(v1.copysign(v2));
        }

        // 类型转换指令
        // part1: 整数截断，共1条指令
        fn i32_wrap_i64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_u64();
            self.operand_stack.push_u32(v as u32);
        }
        // part2: 整数拉升，共7条指令
        fn i64_extend_i32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i32();
            self.operand_stack.push_u64(v as u64);
        }

        fn i64_extend_u32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_u32();
            self.operand_stack.push_u64(v as u64);
        }

        fn i32_extend_8(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i32() as i8;
            self.operand_stack.push_i32(v as i32);
        }

        fn i32_extend_16(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i32() as i16;
            self.operand_stack.push_i32(v as i32);
        }

        fn i64_extend_8(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i64() as i8;
            self.operand_stack.push_i64(v as i64);
        }

        fn i64_extend_16(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i64() as i16;
            self.operand_stack.push_i64(v as i64);
        }

        fn i64_extend_32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i64() as i32;
            self.operand_stack.push_i64(v as i64);
        }
        // part3: 浮点数截断，共9条指令
        fn i32_trunc_f32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f32();
            self.operand_stack.push_i32(v.trunc() as i32);
        }

        fn u32_trunc_f32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f32();
            self.operand_stack.push_u32(v.trunc() as u32);
        }

        fn i32_trunc_f64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f64();
            self.operand_stack.push_i32(v.trunc() as i32);
        }

        fn u32_trunc_f64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f64();
            self.operand_stack.push_u32(v.trunc() as u32);
        }

        fn i64_trunc_f32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f32();
            self.operand_stack.push_i64(v.trunc() as i64);
        }

        fn u64_trunc_f32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f32();
            self.operand_stack.push_u64(v.trunc() as u64);
        }

        fn i64_trunc_f64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f64();
            self.operand_stack.push_i64(v.trunc() as i64);
        }

        fn u64_trunc_f64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f64();
            self.operand_stack.push_u64(v.trunc() as u64);
        }

        // part4: 整数转换，共8条指令
        fn f32_convert_i32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i32();
            self.operand_stack.push_f32(v as f32);
        }

        fn f32_convert_u32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_u32();
            self.operand_stack.push_f32(v as f32);
        }

        fn f32_convert_i64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i64();
            self.operand_stack.push_f32(v as f32);
        }

        fn f32_convert_u64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_u64();
            self.operand_stack.push_f32(v as f32);
        }

        fn f64_convert_i32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i32();
            self.operand_stack.push_f64(v as f64);
        }

        fn f64_convert_u32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_u32();
            self.operand_stack.push_f64(v as f64);
        }

        fn f64_convert_i64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_i64();
            self.operand_stack.push_f64(v as f64);
        }

        fn f64_convert_u64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_u64();
            self.operand_stack.push_f64(v as f64);
        }
        // part5: 浮点数精度调整，共2条指令
        fn f32_demote_f64(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f64();
            self.operand_stack.push_f32(v as f32);
        }

        fn f64_promote_f32(&mut self, _args: &Option<Rc<dyn Any>>) {
            let v = self.operand_stack.pop_f32();
            self.operand_stack.push_f64(v as f64);
        }
        // part6: 比特位重新解释，共4条指令，只需重新解释类型，无需做任何操作
        fn i32_reinterpret_f32(&mut self, _args: &Option<Rc<dyn Any>>) {}
        fn i64_reinterpret_f64(&mut self, _args: &Option<Rc<dyn Any>>) {}
        fn f32_reinterpret_i32(&mut self, _args: &Option<Rc<dyn Any>>) {}
        fn f64_reinterpret_i64(&mut self, _args: &Option<Rc<dyn Any>>) {}

        // 内存相关指令
        // helper function
        fn get_offset(&mut self, args: &Option<Rc<dyn Any>>) -> usize {
            let arg = args.as_ref().unwrap().downcast_ref::<MemArg>().unwrap();
            // 动态的操作数偏移量 + 静态的立即数偏移量，结果可能溢出u32，得用u64表示
            self.operand_stack.pop_u32() as usize + arg.offset as usize
        }

        fn read_u8(&mut self, args: &Option<Rc<dyn Any>>) -> u8 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8];
            self.memory.read(offset, &mut buf[..]);
            buf[0]
        }

        fn read_u16(&mut self, args: &Option<Rc<dyn Any>>) -> u16 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8; 2];
            self.memory.read(offset, &mut buf[..]);
            u16::from_le_bytes(buf.try_into().unwrap())
        }

        fn read_u32(&mut self, args: &Option<Rc<dyn Any>>) -> u32 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8; 4];
            self.memory.read(offset, &mut buf[..]);
            u32::from_le_bytes(buf.try_into().unwrap())
        }

        fn read_u64(&mut self, args: &Option<Rc<dyn Any>>) -> u64 {
            let offset = self.get_offset(args);
            let mut buf = vec![0u8; 8];
            self.memory.read(offset, &mut buf[..]);
            u64::from_le_bytes(buf.try_into().unwrap())
        }

        fn write_u8(&mut self, args: &Option<Rc<dyn Any>>, n: u8) {
            let offset = self.get_offset(args);
            let buf = vec![n];
            self.memory.write(offset, &buf[..]);
        }

        fn write_u16(&mut self, args: &Option<Rc<dyn Any>>, n: u16) {
            let offset = self.get_offset(args);
            let buf = n.to_le_bytes();
            self.memory.write(offset, &buf);
        }

        fn write_u32(&mut self, args: &Option<Rc<dyn Any>>, n: u32) {
            let offset = self.get_offset(args);
            let buf = n.to_le_bytes();
            self.memory.write(offset, &buf);
        }

        fn write_u64(&mut self, args: &Option<Rc<dyn Any>>, n: u64) {
            let offset = self.get_offset(args);
            let buf = n.to_le_bytes();
            self.memory.write(offset, &buf);
        }

        // part1: size 和 grow
        fn memory_size(&mut self, _args: &Option<Rc<dyn Any>>) {
            self.operand_stack.push_u32(self.memory.size() as u32);
        }

        fn memory_grow(&mut self, _args: &Option<Rc<dyn Any>>) {
            let grow_size = self.operand_stack.pop_u32();
            println!("memory grow size = {}", grow_size);
            let old_size = self.memory.grow(grow_size as usize);
            println!(
                "old size = {}, new_size = {}",
                old_size,
                self.memory.size()
            );
            self.operand_stack.push_u32(old_size as u32);
        }

        // part2: load
        fn i32_load(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u32(args);
            self.operand_stack.push_u32(val);
        }

        fn i64_load(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u64(args);
            self.operand_stack.push_u64(val);
        }

        fn f32_load(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u32(args);
            self.operand_stack.push_u32(val);
        }

        fn f64_load(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u64(args);
            self.operand_stack.push_u64(val);
        }

        fn i32_load_8s(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u8(args);
            self.operand_stack.push_i32(val as i8 as i32);
        }

        fn i32_load_8u(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u8(args);
            self.operand_stack.push_u32(val as u32);
        }

        fn i32_load_16s(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u16(args);
            self.operand_stack.push_i32(val as i16 as i32);
        }

        fn i32_load_16u(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u16(args);
            self.operand_stack.push_u32(val as u32);
        }

        fn i64_load_8s(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u8(args);
            self.operand_stack.push_i64(val as i8 as i64);
        }

        fn i64_load_8u(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u8(args);
            self.operand_stack.push_u64(val as u64);
        }

        fn i64_load_16s(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u16(args);
            self.operand_stack.push_i64(val as i16 as i64);
        }

        fn i64_load_16u(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u16(args);
            self.operand_stack.push_u64(val as u64);
        }

        fn i64_load_32s(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u32(args);
            self.operand_stack.push_i64(val as i32 as i64);
        }

        fn i64_load_32u(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.read_u32(args);
            self.operand_stack.push_u64(val as u64);
        }

        // part3: store
        fn i32_store(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u32();
            self.write_u32(args, val);
        }

        fn i64_store(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.write_u64(args, val);
        }

        fn f32_store(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u32();
            self.write_u32(args, val);
        }

        fn f64_store(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.write_u64(args, val);
        }

        fn i32_store_8(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u32();
            self.write_u8(args, val as u8);
        }

        fn i32_store_16(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u32();
            self.write_u16(args, val as u16);
        }

        fn i64_store_8(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.write_u8(args, val as u8);
        }

        fn i64_store_16(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.write_u16(args, val as u16);
        }
        fn i64_store_32(&mut self, args: &Option<Rc<dyn Any>>) {
            let val = self.operand_stack.pop_u64();
            self.write_u32(args, val as u32);
        }

        // 局部变量指令
        fn local_get(&mut self, args: &Option<Rc<dyn Any>>) {
            let idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let val = self
                .operand_stack
                .get_operand(self.local_0_idx + *idx as usize);
            self.operand_stack.push_u64(val);
        }

        fn local_set(&mut self, args: &Option<Rc<dyn Any>>) {
            let idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let val = self.operand_stack.pop_u64();
            self.operand_stack
                .set_operand(self.local_0_idx + *idx as usize, val);
        }

        fn local_tee(&mut self, args: &Option<Rc<dyn Any>>) {
            let idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let val = self.operand_stack.pop_u64();
            self.operand_stack.push_u64(val);
            self.operand_stack
                .set_operand(self.local_0_idx + *idx as usize, val);
        }

        // 全局变量指令
        fn global_get(&mut self, args: &Option<Rc<dyn Any>>) {
            let idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let val = self.globals[*idx as usize].get_as_u64();
            self.operand_stack.push_u64(val);
        }

        fn global_set(&mut self, args: &Option<Rc<dyn Any>>) {
            let idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let val = self.operand_stack.pop_u64();
            self.globals[*idx as usize].set_as_u64(val);
        }

        // 控制指令
        fn br_if(&mut self, args: &Option<Rc<dyn Any>>) {
            if self.operand_stack.pop_bool() {
                self.br(args);
            }
        }

        fn block(&mut self, args: &Option<Rc<dyn Any>>) {
            let block_args =
                args.as_ref().unwrap().downcast_ref::<BlockArgs>().unwrap();
            let block_type = self.module.get_block_type(block_args.block_type);
            self.enter_block(
                OpCode::Block,
                block_type,
                block_args.instructions.clone(),
            );
        }

        fn loop_instr(&mut self, args: &Option<Rc<dyn Any>>) {
            let block_args =
                args.as_ref().unwrap().downcast_ref::<BlockArgs>().unwrap();
            let block_type = self.module.get_block_type(block_args.block_type);
            self.enter_block(
                OpCode::Loop,
                block_type,
                block_args.instructions.clone(),
            );
        }

        fn if_instr(&mut self, args: &Option<Rc<dyn Any>>) {
            let if_args =
                args.as_ref().unwrap().downcast_ref::<IfArgs>().unwrap();
            let block_type = self.module.get_block_type(if_args.block_type);
            let instrs;
            if self.operand_stack.pop_bool() {
                instrs = if_args.instructions_1.clone();
            } else {
                instrs = if_args.instructions_2.clone();
            }
            self.enter_block(OpCode::If, block_type, instrs);
        }

        fn br(&mut self, args: &Option<Rc<dyn Any>>) {
            let label_idx =
                args.as_ref().unwrap().downcast_ref::<BrArgs>().unwrap();
            // 先弹出label_idx 个控制帧
            for _ in 0..*label_idx {
                self.control_stack.pop_control_frame();
            }
            let cf = self.control_stack.top_control_frame();
            if cf.opcode != OpCode::Loop {
                // 如果是 block 或者 if 块，再弹出一个控制帧, 不能直接pop，因为还有参数和返回值要处理
                self.exit_block();
            } else {
                // 如果是 loop 块，需要重新进入进入控制帧
                cf.pc = 0;
                // self.reset_block(cf);
                let mut results = self
                    .operand_stack
                    .pop_u64s(cf.block_type.params_types.len());
                self.operand_stack
                    .pop_u64s(self.operand_stack.length() - cf.bp);
                self.operand_stack.push_u64s(&mut results);
            }
        }

        fn br_table(&mut self, args: &Option<Rc<dyn Any>>) {
            let br_table_args = args
                .as_ref()
                .unwrap()
                .downcast_ref::<BrTableArgs>()
                .unwrap();
            let idx = self.operand_stack.pop_u32() as usize;
            if idx < br_table_args.labels.len() {
                self.br(&Some(Rc::new(br_table_args.labels[idx])));
            }
        }

        fn return_instr(&mut self, _: &Option<Rc<dyn Any>>) {
            let (_, label_idx) = self.control_stack.top_call_frame();
            self.br(&Some(Rc::new(label_idx as BrArgs)));
        }

        fn call_indrect(&mut self, args: &Option<Rc<dyn Any>>) {
            let i = self.operand_stack.pop_u32();
            if self.table.as_ref().is_none() || i > self.table.as_ref().unwrap().size() as u32 {
                panic!("Undefined element");
            }
            let table = self.table.as_ref().unwrap();
            let func_in_table = &table.get_elem(i as usize);
            let type_idx = args.as_ref().unwrap().downcast_ref::<u32>().unwrap();
            let func_type = &self.module.type_sec[*type_idx as usize];
            if func_in_table.func_type.get_signature() != func_type.get_signature() {
                panic!("Indirect call type mismatch");
            }
            if func_in_table.code.is_some() {
                self.call_internal_func(func_in_table);
            } else if func_in_table.native_func.is_some() {
                self.call_external_func(func_in_table);
            } else {
                panic!("Unexpected function type");
            }
        }

        fn unreachable(&mut self, _: &Option<Rc<dyn Any>>) {
            panic!("Unreachable");
        }

        fn nop(&mut self, _: &Option<Rc<dyn Any>>) {
            // do nothing
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

        #[test]
        fn test_local_var() {
            let mut operand_stack = OperandStack::new();
            operand_stack.push_u32(1);
            operand_stack.push_u32(3);
            operand_stack.push_u32(5);
            assert_eq!(3, operand_stack.get_operand(1));
            operand_stack.set_operand(1, 7);
            assert_eq!(7, operand_stack.get_operand(1));
        }

        #[test]
        fn test_global_var() {
            let mut g = GlobalVar::new(
                GlobalType {
                    val_type: ValType::I32,
                    mutable: true,
                },
                0,
            );
            g.set_as_u64(100);
            assert_eq!(100, g.get_as_u64());
        }

        #[test]
        fn test_memory() {
            // test memory size and grow
            let mut mem = Memory::new(Limits { min: 2, max: None });
            assert_eq!(mem.size(), 2);
            assert_eq!(mem.grow(3), 2);
            assert_eq!(mem.size(), 5);
        }
    }
}
