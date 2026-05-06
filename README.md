# Aldebaran

Aldebaran（毕宿五），金牛座中最亮的恒星。一个 [DeepSeek](https://platform.deepseek.com/) 原生的终端 AI 编程助手。

基于 [claw-code](https://github.com/ultraworkers/claw-code) 重构，适配 DeepSeek V4 API。

## 特性

- DeepSeek V4 原生支持 — 100 万 token 上下文窗口、推理模式（reasoning_effort）
- 终端 REPL 交互 — 文件读写、命令执行、Git 操作
- 记忆系统 — 多文件持久化记忆（用户/项目/反馈/参考）
- 会话管理 — 多会话切换、恢复、导出

## 安装

```bash
# 克隆仓库
git clone https://github.com/YOUR_USERNAME/aldebaran.git
cd aldebaran

# 编译
cargo build --release -p taurus-cli

# 安装到 PATH
sudo cp target/release/taurus /usr/local/bin/taurus
```

## 使用

```bash
# 设置 API Key
export DEEPSEEK_API_KEY=sk-xxx

# 启动交互式 REPL
taurus

# 单次问答
taurus prompt "用 Rust 写一个 HTTP 服务器"

# 恢复最近的会话
taurus --resume latest

# JSON 输出（便于脚本集成）
taurus --output-format json prompt "解释这个项目"
```

## 许可证

MIT
