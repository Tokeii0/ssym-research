# 验证记录

**中文** · [English](VALIDATION.en.md) · [项目首页](README.md)

日期：2026-10-05。测试引擎的 build ID：
`1b53c4adc5ec1df68585a5a2912c90b4418c0b92bcfb58d3e1fab79dcb72d994`。
实际输入身份、二进制和源码哈希见 [原始汇总](data/starmem.json)、[源码清单](data/source-manifest.json)。

| 验证 | 结果 | 边界 |
| --- | --- | --- |
| `cargo test ... --release --lib profile::compiled` | 4 passed | 包括字段往返、确定性、所有截断位置、单字节损坏、重算校验和后的非法索引及身份 |
| StarMem 原生 CLI 和 benchmark example release 构建 | 通过 | 可执行文件位于宿主 target；没有分发二进制 |
| LovelyMem 宿主 `cargo check --manifest-path src-tauri/Cargo.toml --locked` | 通过 | 不代表桌面界面、授权服务器或所有插件已验收 |
| `cargo clippy ... --release --lib --example symbol_format_bench` | 退出码 0，45 条现有引擎告警 | 未出现定位到新增 compiled 模块或 example 的告警；没有顺带修改其他模块 |
| 原生 `profile PDB -o kernel.ssym` 与 `pslist --profile kernel.ssym` | 实际运行通过 | 样本 A 输出 63 条，保留部分结果告警 |
| 损坏 SSYM / 错误 GUID 但有效 checksum | 两项均按预期拒绝 | [原生命令结果](data/rejection-checks.json)，不是仅测解析函数 |
| StarMem 三镜像、六种加载方式 | 全部 profile 哈希和查询校验值一致 | 9 次延迟采样；分配统计另跑 |
| StarMem 三镜像、三种会话方式 | 同镜像完整报告哈希一致 | 每项 5 次；A/C 为部分结果 |
| Vol3 三份同源 PDB 的完整 ISF | 生成与加载通过 | 每格式 9 次加载；完整类型图范围不同 |
| Vol3 原生 pslist | A 零条；B 初始化失败；C 161 条 | 保留不等价结果，不宣称跨引擎正确性通过 |
| StarMem vendor 来源清单 | PowerShell 7 下校验通过 | 清单含共享工作区其他已有改动，不代替本次测试 build ID |
| 研究接入补丁 | 对父版本 main.rs / session.rs 的 `git apply --check` 通过 | 其他版本仍需先审阅上下文 |

本研究只验证 Windows x64 内核 profile 与 pslist。尚未覆盖其他插件、其他模块 PDB、Linux、
x86 PAE、冷盘、更多硬件与完整 ISF 的通用类型图。现有 SSYM 接入不改变默认符号路径。

发布包自身可以离线核对，既不需要镜像，也不需要授权：

```powershell
python -B -X utf8 ssym-research/scripts/verify-study.py
```

该命令核对源码哈希、原始采样与汇总、JSON/SSYM 内容一致标记、会话哈希、Vol3 的 PDB 身份、
文档链接和数据中的本机绝对路径；不替代重新运行取证引擎或对原始证据的独立复核。

本次双语文档整理不改变实测源码和采样，也没有重新执行引擎基准。
源码测试和性能结果属于上述实验版本；文档检查覆盖双语配对、表格数值、示例命令与链接。

9 组中英文文档的配对、数值表格及命令一致性检查通过；10 个 Mermaid 代码块使用
Mermaid 11.12.0 在本地无界面浏览器中完成语法检查与渲染，关键布局已查看。
这是本地文档验证，不是 GitHub 服务端发布或渲染结果。磁盘布局另以等宽文本图表达。
发布许可为 [Apache-2.0](LICENSE-APACHE)，完整 StarMem 引擎源码仍未公开。
