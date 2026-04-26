# Test 文件夹说明

这个文件夹包含了项目开发过程中的测试文件、文档和工具脚本，不影响项目主体运行。

## 目录结构

### 📁 data/
测试数据文件，用于开发和测试文件处理功能：
- `test_data.csv` - CSV 测试数据
- `test_file_1.csv` - 测试文件 1（包含姓名、电话、邮箱、身份证号）
- `test_file_2.csv` - 测试文件 2（包含用户名、手机号、地址、银行卡号）
- `test_file_3.md` - Markdown 测试文件
- `test_markdown.md` - Markdown 格式测试
- `test_sensitive_doc.md` - 敏感信息测试文档

### 📁 docs/
项目开发过程中的技术文档和总结：
- `APPLICATION_RENAME_SUMMARY.md` - 应用重命名总结
- `CRASH_DEBUGGING_SUMMARY.md` - 崩溃调试记录
- `CROSS_PLATFORM_PATH_IMPLEMENTATION.md` - 跨平台路径实现文档
- `DATABASE_IMPLEMENTATION_SUMMARY.md` - 数据库实现总结
- `LOG_LAYOUT_OPTIMIZATION_SUMMARY.md` - 日志布局优化总结
- `generate-icons.md` - 图标生成说明

### 📁 scripts/
构建和工具脚本：
- `build-with-icons.ps1` - 带图标的构建脚本
- `convert-logo.ps1` - Logo 转换脚本
- `create-proper-ico.ps1` - 创建 ICO 文件脚本
- `final-build-solution.ps1` - 最终构建解决方案
- `fix-ico-and-build.ps1` - 修复图标并构建
- `generate-icons.ps1` - 图标生成脚本

### 📁 installers/
历史构建的安装包（仅供参考）：
- `CheersAI-Vault-Installer.msi` - MSI 安装包
- `CheersAI-Vault-Setup.exe` - NSIS 安装程序

## 注意事项

- 这个文件夹中的所有内容都不会影响项目的正常运行
- 测试数据文件可以用于本地开发测试
- 文档记录了项目开发过程中的重要决策和实现细节
- 脚本文件可以用于重新生成图标或构建安装包
