# 添加 Cargo 到 PATH
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# 启动 Tauri 开发服务器
pnpm tauri dev
