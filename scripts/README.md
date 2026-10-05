# 研究工具

**中文** · [English](README.en.md) · [项目首页](../README.md)

本目录脚本支撑同一个研究流程，完整命令见 [复现指南](../REPRODUCE.md)。
普通脚本使用 Python 标准库；Vol3 脚本还需要本地可用的 Volatility 3 环境。

| 工具 | 输入 | 输出与作用 |
| --- | --- | --- |
| [benchmark-starmem.py](benchmark-starmem.py) | 授权后的 benchmark EXE、镜像、可选符号根 | 顺序运行独立进程；准备等价格式；预热、分配和延迟测量；保存报告与统计 |
| [benchmark-vol3.py](benchmark-vol3.py) | StarMem 原始 results.json、本地 Vol3 | 同 PDB 生成完整 ISF；隔离符号目录；测加载和原生 pslist；保留失败 |
| [check-ssym-rejection.py](check-ssym-rejection.py) | 原生 CLI、镜像、有效 SSYM | 构造损坏与错误 GUID 文件，检查真实会话按预期拒绝 |
| [export-study.py](export-study.py) | 实验仓库和本机运行目录 | 允许清单导出源码及统计；脱敏路径；保留 Apache-2.0；不覆盖研究文档 |
| [verify-study.py](verify-study.py) | 当前发布目录 | 离线检查源码哈希、统计、语言配对、表格、命令、链接和数据路径 |

基准与拒绝检查要求新的输出目录，避免混合不同实验。`export-study.py` 会更新本发布目录中的
数据和参考源码，只有输入目录属于同一实测版本时才应执行。文档翻译不需要重新导出或重跑基准。
导出原始 stderr 便于诊断，但发布前仍需复核其中是否存在本机或样本特有信息。

线图源码保存在 Markdown 的 Mermaid 代码块内；字节布局使用等宽文本图。
编辑格式时按 [格式规范](../FORMAT.md) 与参考实现交叉检查，而不是从示意图推断新的字段含义。
代码和数据保持原始标识符，说明文档提供双语版本。
