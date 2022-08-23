mod dumper;
mod module;
mod interpreter;

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Dump the input file
    #[clap(short, long, value_parser)]
    dump: bool,

    /// The input wasm file
    #[clap(short, long, value_parser)]
    file: String,
}

fn main() {
    let args = Args::parse();
    let module = module::WasmReader::decode_file(args.file).unwrap();
    if args.dump {
        dumper::Dumper::dump(&module);
    } else {
        interpreter::VM::exec_main(&module);
    }
    
    
}
