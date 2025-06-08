fn main() -> std::io::Result<()> {
    // 获取OUT_DIR环境变量，prost_build会把生成的代码放在这个目录下
    // let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    // println!("cargo:warning=Proto files will be compiled into: {}", out_dir);

    // 指定.proto文件的路径和输出目录
    // prost_build::compile_protos(&["src/protos/trade_protocol.proto"], &["src/protos/"])?;
    
    // 更常见的做法是直接指定编译目标目录为 OUT_DIR 下的某个子路径，然后在代码中通过 include! 宏引入
    // 这样可以避免将生成的文件直接放入 src 目录，保持源码目录的清洁
    prost_build::Config::new()
        .out_dir("src/protos_generated") // 指定生成代码的输出目录
        .compile_protos(&["src/protos/trade_protocol.proto"], &["src/protos/"])?;
    Ok(())
}
