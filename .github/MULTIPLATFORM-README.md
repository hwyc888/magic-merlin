# MagicTier 多平台 Core + CLI

## 选择平台

| 目录 | 系统与处理器 | 主程序 | 管理命令行 |
| --- | --- | --- | --- |
| windows-x64 | 64 位 Windows，Intel/AMD x64 | magictier-core.exe | magictier-cli.exe |
| linux-x64 | Linux x86_64，musl 静态链接 | magictier-core | magictier-cli |
| linux-arm64 | Linux AArch64 / ARM64，musl 静态链接 | magictier-core | magictier-cli |
| linux-armv7 | Linux ARMv7 hard-float，musl 静态链接 | magictier-core | magictier-cli |

这些是命令行可执行程序，不是 Windows 图形界面，也不是路由器插件安装包。路由器插件继续由独立的 router-build 工作流生成。

## 功能优先的 CLI 体积优化

Core 和 CLI 均保留项目原有的全部默认编译特性：wireguard、websocket、smoltcp、tun、socks5、quic。没有使用 --no-default-features，没有为了减小文件删除命令或协议支持，也没有改变 CLI 的线程调度方式。

Core 使用 release 配置；CLI 使用 cli-small 配置，仅进行 opt-level=z、fat LTO、单代码生成单元、移除符号等编译优化。CLI 保留与默认 release 相同的 panic=unwind 行为。不使用 UPX 加壳，也不通过强制内存上限让命令提前退出。

CLI 是管理工具，执行命令后退出；保持组网的是 Core。文件大小与运行内存不同，不能把二进制缩小比例当作内存节省比例。各平台 BUILD-REPORT.json 提供二进制大小、校验值和实测范围；Linux x64 报告的 peak_rss 是帮助命令和本地节点查询的进程峰值 RSS，单位 KiB，不代表高负载 Core 的内存占用。

## Windows 使用

保留 windows-x64 目录内全部文件。Packet.dll、wintun.dll、WinDivert64.sys 必须与程序一起分发，不要只复制两个 EXE。创建虚拟网卡、安装服务或操作驱动时使用管理员权限；依赖具体网卡抓包能力的功能还受系统已安装的驱动和权限影响。

在 PowerShell 中进入该目录：

```powershell
.\magictier-core.exe --zzhelp
.\magictier-cli.exe --help
```

在一个终端运行 Core，使用你自己的网络名、密钥和对端参数；需要本机管理时显式设置 `--rpc-portal 127.0.0.1:15888`。在另一个终端查询：

```powershell
.\magictier-cli.exe -p 127.0.0.1:15888 node
.\magictier-cli.exe -p 127.0.0.1:15888 peer
.\magictier-cli.exe -p 127.0.0.1:15888 -o json route
.\magictier-cli.exe service --help
```

## Linux 使用

先用 `uname -m` 确认处理器架构，再选择对应目录。ZIP 解压后可能需要恢复执行权限：

```sh
chmod +x magictier-core magictier-cli
./magictier-core --zzhelp
./magictier-cli --help
```

Core 访问 TUN 和配置网络通常需要 root 权限及系统提供 `/dev/net/tun`。运行 Core 时显式启用本地管理端口 `--rpc-portal 127.0.0.1:15888`，然后查询：

```sh
./magictier-cli -p 127.0.0.1:15888 node
./magictier-cli -p 127.0.0.1:15888 peer
./magictier-cli -p 127.0.0.1:15888 -o json route
./magictier-cli service --help
```

不要把管理 RPC 端口直接暴露到公网。网络名、密钥和服务端地址使用你自己的配置，测试用密钥不能用于部署。

## 校验与测试范围

根目录 SHA256SUMS.txt 用于验证文件完整性。Linux 在包根目录运行 `sha256sum -c SHA256SUMS.txt`。四个平台使用同一次构建解析出的同一份 Cargo.lock，保存在各平台目录；重新编译时把它放回 magictier/Cargo.lock 并使用 --locked。

自动验证包含 Core 启动帮助/版本、优化 CLI 与标准 release CLI 的 16 类命令帮助一致性、Bash/PowerShell 补全一致性、错误参数退出码。Windows x64 和 Linux x64 另外使用临时的本地 Core 验证查询及配置增删；测试使用 no-TUN 模式，不更改生产网络或安装服务。ARM64/ARMv7 使用 QEMU 验证启动和命令界面，不能替代真实 ARM 设备的联网验收。

这些检查不是所有网络场景的完整验收。真实 TUN、系统服务安装、跨主机 NAT 穿透、RDP、长期高负载和具体设备驱动兼容性仍需在目标设备验证。原有命令的实现范围保持不变，帮助一致性不表示补齐了原项目中未实现的功能。

注意：本项目 Core 原有的帮助和版本参数是 `--zzhelp`、`--zzversion`；CLI 使用标准的 `--help`、`--version`。此次保留 Core 原有参数命名，没有更改运行逻辑。
