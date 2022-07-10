mod dumper;
mod module;

fn main() {
    let module = module::WasmReader::decode_file("data/hw_rust.wasm").unwrap();
    dumper::Dumper::dump(&module);
}
