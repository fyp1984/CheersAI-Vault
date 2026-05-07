# FileBay 配置数据库存储方案

## 问题
Token 无法通过 JSON 文件可靠地保存，需要使用数据库存储。

## 解决方案

### 1. 数据库表已创建
在 `src-tauri/src/core/database.rs` 中已添加：
- `filebay_config` 表
- `FileBayConfig` 结构体
- 数据库操作方法

### 2. 简化的实现方案

不在应用启动时初始化数据库，而是在每个命令中按需初始化：

```rust
// 在 sync_filebay_config_from_desktop 中
let db = Database::new().await.map_err(|e| format!("数据库初始化失败: {}", e))?;
db.save_filebay_config(&url, &token, &owner, &repo, true).await?;
```

### 3. 需要修改的文件

1. **src-tauri/src/commands/gitea.rs**
   - 移除 `app: AppHandle` 参数中对数据库状态的依赖
   - 在命令内部直接初始化数据库
   
2. **src-tauri/src/lib.rs**
   - 移除数据库初始化代码
   - 保持简单的 setup 函数

### 4. 实现步骤

```rust
// gitea.rs 中的修改
#[tauri::command]
pub async fn sync_filebay_config_from_desktop(
    state: State<'_, GiteaState>,
    url: String,
    token: String,
    owner: String,
    repo: String,
) -> Result<String, String> {
    // 初始化数据库
    let db = Database::new().await
        .map_err(|e| format!("数据库初始化失败: {}", e))?;
    
    // 保存到数据库
    db.save_filebay_config(&url, &token, &owner, &repo, true).await
        .map_err(|e| format!("保存配置失败: {}", e))?;
    
    // 更新内存中的配置
    let mut config = state.config.lock().await;
    config.url = url.clone();
    config.token = token.clone();
    config.owner = owner.clone();
    config.repo = repo.clone();
    config.enabled = true;
    
    Ok(format!("✅ 配置已保存\n用户: {}\n仓库: {}\nToken: {}字符", owner, repo, token.len()))
}
```

## 优势
1. 简单可靠 - 不依赖复杂的应用状态管理
2. 每次都能确保数据库可用
3. 不需要修改 GiteaState 的结构
4. 向后兼容 - JSON 文件仍然作为备份

## 测试
1. 启动应用
2. 在 Desktop 页面下载配置文件
3. 检查控制台日志确认数据库保存成功
4. 刷新 FileBay 设置页面，确认 Token 已保存
