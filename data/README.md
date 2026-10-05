# 数据与来源说明

**中文** · [English](README.en.md) · [项目首页](../README.md)

本目录保存 2026-10-05 实验的统计及来源元数据。没有镜像、PDB、完整符号内容、进程明细或授权文件。
所有本机路径替换为 `${IMAGE_A}`、`${PDB_A}`、`${CASE_A}` 等标识；这些是脱敏占位符，不是可直接运行的路径。

| 文件 | 内容 | 解释边界 |
| --- | --- | --- |
| [starmem.json](starmem.json) | 三样本身份、产物哈希、汇总、引擎 build ID | `manifest` 包含单次准备阶段计时，不是重复采样中位数 |
| [starmem-commands.json](starmem-commands.json) | 调用参数、退出码、stderr 和总墙钟时间 | 墙钟含启动等开销，与内部加载计时不同 |
| `image-A/`、`image-B/`、`image-C/` | load-samples、session-samples、report-status | 完整报告本身不分发，仅保留哈希和状态 |
| [vol3.json](vol3.json) | 完整 ISF 转换、加载、会话和集合差异数量 | 非相同语义模型；失败与零条不等于成功完成同一任务 |
| [vol3-commands.json](vol3-commands.json) | 原始命令与诊断 | 保留失败；本机路径和原文件名已替换 |
| [environment.json](environment.json) | 实验日期、硬件和系统 | 单机环境，不代表跨平台结果 |
| [source-manifest.json](source-manifest.json) | 实测参考文件的 SHA-256 与父 commit | 父 commit 不包括原工作树已有修改 |
| [rejection-checks.json](rejection-checks.json) | 原生会话拒绝错误 SSYM 的检查 | `passed=true` 表示按预期失败，不是完成取证分析 |

## 单位与状态

- `load_us` 为微秒；报告表格换算为毫秒。`lookup_pair_ns` 为每次字段/符号查询对的纳秒数。
- `open_ms`、`pslist_ms`、`total_ms` 是内部阶段耗时；`process_wall_ms` 或命令墙钟另含进程开销。
- `peak_heap_bytes` / `retained_heap_bytes` 是 Rust 请求堆；`python_traced_*` 是 tracemalloc 跟踪内存。两者都不是 RSS。
- 汇总的 `median/min/max/samples` 来自全部正式采样。`heaps` 与 `warmups` 不混入延迟统计。
- `complete=false` 表示引擎报告不完整。`report-status.json` 公开告警种类，移除了地址和完整消息。
- `session_reports_equal=true` 仅说明同一镜像不同符号路径的完整序列化报告一致。
- Vol3 比较中的 `equal=null` 与 `status=failed` 表示未取得可比较输出，不应折叠为零条。

## 原始数据与双语说明

JSON 字段名、原始诊断和数值保持采样语义，不另生成翻译后的第二套测量数据。
中英文 [基准报告](../BENCHMARKS.md) / [benchmark report](../BENCHMARKS.en.md) 解释同一份数据。
源码的原始包元数据按哈希保留，研究发布包采用 [Apache-2.0](../LICENSE-APACHE)。

从研究根目录执行 `python -B -X utf8 scripts/verify-study.py` 可核对采样与汇总、哈希、
文档对应和数据路径。它不访问原始镜像，不能证明取证证据完整或复现本次引擎执行。
