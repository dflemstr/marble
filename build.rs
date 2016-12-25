extern crate vulkano_shaders;

fn main() {
    // building the shaders used in the examples
    vulkano_shaders::build_glsl_shaders([
        ("src/teapot.vs.glsl", vulkano_shaders::ShaderType::Vertex),
        ("src/teapot.fs.glsl", vulkano_shaders::ShaderType::Fragment),
    ].iter().cloned());
}
