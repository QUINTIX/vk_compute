use std::process::Command;

const SHADER_LOCATION : &str =  "src/shader.comp";

fn main(){
	Command::new("glslc").args(&[
		"-c", SHADER_LOCATION, "-o", "./compute.spv"
	]).status().unwrap();

	println!("cargo:rerun-if-changed={}", SHADER_LOCATION);
}
