# KovaaK Pulse

轻量级 KovaaK's 成绩监听与复盘工具。Windows 单文件 `.exe`（约 7 MB，常驻内存 ~24 MB），打完一局后自动解析成绩并刷新图表。

## 功能

- **单局复盘**：展示总分、命中率、KPS、平均 TTK、一枪率，以及逐秒双轴表现图、前后半程对比和击杀间隔散点图。
- **深度洞察**：补充游戏内不显示的统计项，包括零失误理论得分上限、最长一枪连杀、连续失误统计、击杀间隔分位数（P10 / P50 / P90）与三段式节奏画像。
- **长期趋势**：按时间范围与指标查看历史走势，包含移动均线（MA5 / MA10 / MA20）、历史 PB 阶梯线、保底线（P25）与常态上限（P90），并支持按灵敏度（`cm/360`）与 FOV 分组对比。
- **训练总览**：18 周训练热力图、今日训练统计、近期 PB 列表（点击可跳转对应对局）与单项目 CSV 导出。
- **轻量常驻**：紧凑窄窗口布局（380 × 560），支持中英双语与深浅色主题切换；后台空载不占 CPU，关闭窗口可隐藏到系统托盘，刷新时不抢游戏焦点。

## 使用

从 [Releases](https://github.com/b53v/KovaaK-Pulse/releases) 下载 `kovaak-pulse.exe` 直接运行即可。

程序会自动识别 Steam 与 KovaaK's 的本地数据目录；如果安装在非标准路径，可在设置中手动指定 `stats` 与 `performances` 文件夹。

## 构建

需要 Windows 10/11 (x64)、Rust (`x86_64-pc-windows-msvc`) 与 Visual Studio 2022 C++ Build Tools。

```powershell
# 运行测试
cargo test

# 编译 Release 可执行文件
cargo build --release
```

编译产物位于 `target\release\kovaak-pulse.exe`。

## License

[LICENSE](LICENSE)
