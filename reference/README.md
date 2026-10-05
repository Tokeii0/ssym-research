# 实现快照

**中文** · [English](README.en.md) · [项目首页](../README.md)

**StarMem 当前尚未公开源代码。**这些片段仅用于解释本次 SSYM 验证，不构成引擎源码发布或公开 SDK。

本目录只保存与实验有关的源文件、依赖快照和接入补丁，不是完整的可构建 StarMem crate。
`Cargo.toml` / `Cargo.lock` 用于记录实测依赖，不应在本目录直接运行 Cargo。

核心文件：

- `src/profile.rs`：本次实测的 PDB → `KernelProfile` 提取器（包含 `pub mod compiled`）。
- `src/profile/compiled.rs`：SSYM 编码、校验、查询与兼容结构还原。
- `src/profile/compiled/tests.rs`：边界及等价测试。
- `examples/symbol_format_bench.rs`：实际 StarMem API 基准入口，保留授权检查。
- `integration.patch`：主 CLI 和会话的可选接入修改，不包含本次研究之外的仓库改动。

在完整 StarMem 源码中合入时，逐项比较提取器差异，再添加 compiled 模块和 example，
审阅并应用接入补丁。不要整目录覆盖正在开发的引擎。
补丁使用实验仓库的上下文；其他版本可能需要手工调整。

`data/source-manifest.json` 保存逐文件哈希。实际二进制的 build ID 记录在基准数据中。
本次共享工作区包含已有未提交修改，父 Git commit 单独不足以复现整个引擎；
精确复现需要保存对应完整 StarMem 源码快照及已有有效授权环境。
发布目录根部的 `.gitattributes` 禁止 Git 对 reference 文件做换行转换，以保留测试源码的字节哈希。
提取配方按 `profile.rs` 原始字节计算；跨系统重新格式化或转换换行后需要重新生成 SSYM。

本研究包按 StarMem 提供的 Apache-2.0 许可选项分发所附代码，见 [LICENSE-APACHE](../LICENSE-APACHE)。
`Cargo.toml` 保留实测源包的原始许可表达式作为来源记录，不改变本研究包选择的 Apache-2.0 许可。
本目录没有复制 Volatility 3 的实现源码；Vol3 对照由本地安装的官方接口运行。
