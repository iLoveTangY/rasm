pub mod dumper {

    use crate::module::*;

    pub struct Dumper<'a> {
        module: &'a Module,
        imported_func_count: i32,
        imported_table_count: i32,
        imported_mem_count: i32,
        imported_global_count: i32,
    }

    impl<'a> Dumper<'a> {
        pub fn dump(module: &Module) {
            let mut d = Dumper {
                module,
                imported_func_count: 0,
                imported_table_count: 0,
                imported_mem_count: 0,
                imported_global_count: 0,
            };

            println!("Version: {:X}", module.version);
            d.dump_type_sec();
            d.dump_import_sec();
            d.dump_func_sec();
            d.dump_table_sec();
            d.dump_mem_sec();
            d.dump_global_sec();
            d.dump_export_sec();
            d.dump_start_sec();
            d.dump_elem_sec();
            d.dump_code_sec();
            d.dump_data_sec();
            d.dump_custom_sec();
        }

        fn dump_type_sec(&self) {
            println!("Type[{}]:", self.module.type_sec.len());
            for (index, func_type) in self.module.type_sec.iter().enumerate() {
                println!("  type[{}]: {}", index, func_type);
            }
        }

        fn dump_import_sec(&mut self) {
            println!("Import[{}]:", self.module.import_sec.len());
            for import in self.module.import_sec.iter() {
                match &(import.desc) {
                    ImportDesc::Func(func) => {
                        println!(
                            "  func[{}]: {}.{}, sig={}",
                            self.imported_func_count,
                            import.module_name,
                            import.member_name,
                            func
                        );
                        self.imported_func_count += 1;
                    }
                    ImportDesc::Table(table) => {
                        println!(
                            "  table[{}]: {}.{}, {}",
                            self.imported_table_count,
                            import.module_name,
                            import.member_name,
                            table
                        );
                        self.imported_table_count += 1;
                    }
                    ImportDesc::Mem(mem) => {
                        println!(
                            "  memory[{}]: {}.{}, {}",
                            self.imported_mem_count,
                            import.module_name,
                            import.member_name,
                            mem
                        );
                        self.imported_mem_count += 1;
                    }
                    ImportDesc::Global(global) => {
                        println!(
                            " global[{}]: {}.{}, {}",
                            self.imported_global_count,
                            import.module_name,
                            import.member_name,
                            global
                        );
                        self.imported_global_count += 1;
                    }
                }
            }
        }

        fn dump_func_sec(&self) {
            println!("Function[{}]:", self.module.func_sec.len());
            for (index, sig) in self.module.func_sec.iter().enumerate() {
                println!(
                    "  func[{}]: sig = {}",
                    self.imported_func_count as usize + index,
                    sig
                );
            }
        }

        fn dump_table_sec(&self) {
            println!("Table[{}]:", self.module.table_sec.len());
            for (index, table) in self.module.table_sec.iter().enumerate() {
                println!(
                    "  table[{}]: {}",
                    self.imported_table_count as usize + index,
                    table.limits
                );
            }
        }

        fn dump_mem_sec(&self) {
            println!("Memory[{}]:", self.module.mem_sec.len());
            for (index, limits) in self.module.mem_sec.iter().enumerate() {
                println!(
                    "  memory[{}]: {}",
                    self.imported_mem_count as usize + index,
                    limits
                );
            }
        }

        fn dump_global_sec(&self) {
            println!("Global[{}]:", self.module.global_sec.len());
            for (index, global) in self.module.global_sec.iter().enumerate() {
                println!(
                    "  global[{}]: {}",
                    self.imported_global_count as usize + index,
                    global.global_type
                );
            }
        }

        fn dump_export_sec(&self) {
            println!("Export[{}]:", self.module.export_sec.len());
            for exp in self.module.export_sec.iter() {
                match exp.desc {
                    ExportDesc::Func(func) => {
                        println!("  func[{}]: name = {}", func, exp.name);
                    }
                    ExportDesc::Table(table) => {
                        println!("  table[{}]: name = {}", table, exp.name);
                    }
                    ExportDesc::Mem(mem) => {
                        println!("  memory[{}]: name = {}", mem, exp.name);
                    }
                    ExportDesc::Global(global) => {
                        println!("  global[{}]: name = {}", global, exp.name);
                    }
                }
            }
        }

        fn dump_start_sec(&self) {
            println!("Start: ");
            if self.module.start_sec.is_some() {
                println!("  func = {}", self.module.start_sec.unwrap());
            } else {
                println!("  none");
            }
        }

        fn dump_elem_sec(&self) {
            println!("Element[{}]:", self.module.elem_sec.len());
            for (index, elem) in self.module.elem_sec.iter().enumerate() {
                println!("  elem[{}]: table = {}", index, elem.table);
            }
        }

        fn dump_code_sec(&self) {
            println!("Code[{}]:", self.module.code_sec.len());
            for (index, code) in self.module.code_sec.iter().enumerate() {
                print!(
                    "  fun[{}]: locals = [",
                    self.imported_func_count as usize + index
                );
                for (index, local) in code.locals.iter().enumerate() {
                    if index > 0 {
                        print!(", ");
                    }
                    print!("{} x {}", local.val_type, local.n);
                }
                println!("]");
                self.dump_expr("    ", &code.expr);
            }
        }

        fn dump_data_sec(&self) {
            println!("Data[{}]:", self.module.data_sec.len());
            for (index, data) in self.module.data_sec.iter().enumerate() {
                println!("  data[{}]: mem = {}", index, data.mem);
            }
        }

        fn dump_custom_sec(&self) {
            println!("Custom[{}]:", self.module.custom_sec.len());
            for (index, cs) in self.module.custom_sec.iter().enumerate() {
                println!("  custom[{}]: name = {}", index, cs.name);
            }
        }

        fn dump_expr(&self, indentation: &str, expr: &Expr) {
            for instruction in expr {
                match instruction.opcode {
                    OpCode::Block | OpCode::Loop => {
                        let args = &instruction.args;
                        let block_args = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<BlockArgs>()
                            .unwrap();
                        let block_type =
                            self.module.get_block_type(block_args.block_type);
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            block_type
                        );
                        self.dump_expr(
                            (indentation.to_owned() + "  ").as_ref(),
                            &block_args.instructions,
                        );
                        println!("{}{}", indentation, "end");
                    }
                    OpCode::If => {
                        let args = &instruction.args;
                        let block_args = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<IfArgs>()
                            .unwrap();
                        let block_type =
                            self.module.get_block_type(block_args.block_type);
                        println!("{}{} {}", indentation, "if", block_type);
                        self.dump_expr(
                            (indentation.to_owned() + "  ").as_ref(),
                            &block_args.instructions_1,
                        );
                        println!("{}{}", indentation, "else");
                        self.dump_expr(
                            (indentation.to_owned() + "  ").as_ref(),
                            &block_args.instructions_2,
                        );
                        println!("{}{}", indentation, "end");
                    }
                    OpCode::Br
                    | OpCode::BrIf
                    | OpCode::LocalGet
                    | OpCode::LocalSet
                    | OpCode::LocalTee
                    | OpCode::GlobalGet
                    | OpCode::GlobalSet
                    | OpCode::Call
                    | OpCode::CallIndirect => {
                        let args = &instruction.args;
                        let param = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<u32>()
                            .unwrap();
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            param
                        );
                    }
                    OpCode::BrTable => {
                        let args = &instruction.args;
                        let param = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<BrTableArgs>()
                            .unwrap();
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            param
                        );
                    }
                    OpCode::MemorySize
                    | OpCode::MemoryGrow
                    | OpCode::TruncSat => {
                        let args = &instruction.args;
                        let param = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<u8>()
                            .unwrap();
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            param
                        );
                    }
                    OpCode::I32Const => {
                        let args = &instruction.args;
                        let param = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<i32>()
                            .unwrap();
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            param
                        );
                    }
                    OpCode::I64Const => {
                        let args = &instruction.args;
                        let param = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<i64>()
                            .unwrap();
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            param
                        );
                    }
                    OpCode::F32Const => {
                        let args = &instruction.args;
                        let param = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<f32>()
                            .unwrap();
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            param
                        );
                    }
                    OpCode::F64Const => {
                        let args = &instruction.args;
                        let param = args
                            .as_ref()
                            .unwrap()
                            .downcast_ref::<f64>()
                            .unwrap();
                        println!(
                            "{}{} {}",
                            indentation,
                            instruction.get_op_name(),
                            param
                        );
                    }
                    _ => {
                        if instruction.opcode >= OpCode::I32Load
                            && instruction.opcode <= OpCode::I64Store32
                        {
                            let args = &instruction.args;
                            let mem_arg = args
                                .as_ref()
                                .unwrap()
                                .downcast_ref::<MemArg>()
                                .unwrap();
                            println!(
                                "{}{} {}",
                                indentation,
                                instruction.get_op_name(),
                                mem_arg
                            );
                        } else {
                            println!(
                                "{}{}",
                                indentation,
                                instruction.get_op_name()
                            );
                        }
                    }
                }
            }
        }
    }
}
