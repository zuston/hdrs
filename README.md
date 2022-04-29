# hdrs &emsp; [![Build Status]][actions] [![Latest Version]][crates.io]

[Build Status]: https://img.shields.io/github/workflow/status/Xuanwo/hdrs/CI/main
[actions]: https://github.com/Xuanwo/hdrs/actions?query=branch%3Amain
[Latest Version]: https://img.shields.io/crates/v/hdrs.svg
[crates.io]: https://crates.io/crates/hdrs

HDFS Native Client in Rust based on [hdfs-sys](https://github.com/Xuanwo/hdfs-sys).

## Quick Start

```rust
use std::io::{Read, Write};

use hdrs::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fs = Client::connect("default")?;
    
    let mut f = fs
        .open("/tmp/hello.txt", libc::O_WRONLY | libc::O_CREAT)?;
    let n = f.write("Hello, World!".as_bytes())?;
    
    let mut f = fs.open("/tmp/hello.txt", libc::O_RDONLY)?;
    let mut buf = vec![0; 1024];
    let n = f.read(&mut buf)?;
    
    let _ = fs.delete("/tmp/hello.txt", false)?;

    Ok(())
}
```

## Compiletime

`hdrs` depends on [hdfs-sys](https://github.com/Xuanwo/hdfs-sys) which links `libjvm` and `libhdfs` to work.

Please make sure `JAVA_HOME`, `HADOOP_HOME` and `LD_LIBRARY_PATH` is set correctly:

```shell
export JAVA_HOME=/path/to/java
export HADOOP_HOME=/path/to/hadoop
export LD_LIBRARY_PATH=${HADOOP_HOME}/lib/native:${JAVA_HOME}/lib/server:${LD_LIBRARY_PATH}
```

## Runtime

`hdrs` depends on [hdfs-sys](https://github.com/Xuanwo/hdfs-sys) which uses JNI to call functions provided by jars that provided by hadoop releases. 

Please also make sure `CLASSPATH` is set correctly during runtime:

```shell
export CLASSPATH=${HADOOP_HOME}/share/hadoop/common/*:${HADOOP_HOME}/share/hadoop/common/lib/*:${HADOOP_HOME}/share/hadoop/hdfs/*:${HADOOP_HOME}/share/hadoop/hdfs/lib/*:${HADOOP_HOME}/etc/hadoop/*
```

## Contributing

Check out the [CONTRIBUTING.md](./CONTRIBUTING.md) guide for more details on getting started with contributing to this project.

## Getting help

Submit [issues](https://github.com/Xuanwo/hdrs/issues/new/choose) for bug report or asking questions in [discussion](https://github.com/Xuanwo/hdrs/discussions/new?category=q-a).

## Acknowledgment

This project is highly inspired by [clang-sys](https://github.com/KyleMayes/clang-sys)

#### License

<sup>
Licensed under <a href="./LICENSE">Apache License, Version 2.0</a>.
</sup>