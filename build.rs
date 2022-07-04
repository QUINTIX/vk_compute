use std::process::Command;

const SHADER_LOCATION : &str =  "src/shader.comp";

fn main(){
	Command::new("glslangValidator").args(&[
		"-o", "./compute.spv", "-V100", SHADER_LOCATION
	]).status().unwrap();

	println!("cargo:rerun-if-changed={}", SHADER_LOCATION);
}
