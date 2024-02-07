# p2p_helper

rust p2p的工具类

并且将rust绑定到python，输出一个whl，python可以直接用

这里的main.rs和peer_id.py都是测试方法

p2p.rs是rust主要用的

lib.rs是封装给python的

运行rust main.rs命令
```shell
cargo run
```

编译为whl命令, 需要在py提前安装maturin
```shell
maturin build
```