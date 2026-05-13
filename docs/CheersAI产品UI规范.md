# CheersAI 产品 UI / UE 规范（Skills 文档）

版本：v1.0（基于 CheersAI Desktop 完整开发文档 v1.0 提取整理）\
适用范围：CheersAI Desktop（Electron + React 18 + Tailwind CSS）所有界面开发、配色与交互实现\
权威性说明：本规范为统一实现口径；如与实现冲突，以本规范为准并同步回写源规范文档\
来源：cheersai-dev-doc.md（“设计规范 / 应用结构 / 核心组件 / 功能模块界面 / 交互流程 / 图标索引 / 颜色参考”等章节）

***

## 1. UE 目标与交互原则

### 1.1 产品体验目标

- 安全可信：持续显性传达“隐私保护/脱敏/审计不可篡改”等安全状态与反馈。
- 高一致性：色彩、排版、间距、组件状态、动效时长在全局保持一致。
- 高可用性：信息层级清晰，操作路径短，关键操作有明确反馈与可撤销/可解释提示。

### 1.2 交互原则（必须遵循）

- 反馈及时：交互反馈优先使用 150ms–200ms 的过渡；处理中状态必须可见（进度条/旋转/输入中动画）。
- 状态可辨：未选中/悬浮/选中/禁用等状态必须有明确视觉差异。
- 信息可扫读：列表/表格提供清晰的列标题、对齐规则与 hover 行反馈。
- 关键流程可追溯：脱敏、模型调用、还原等关键操作应伴随审计/日志反馈与固定提示条。

***

## 2. 色彩体系

### 2.1 色彩变量（必须统一）

#### 主色（品牌色）

```css
--primary-blue: #3b82f6;       /* 主蓝色 */
--primary-blue-dark: #2563eb;  /* 深蓝色 */
--primary-blue-light: #60a5fa; /* 浅蓝色 */
```

#### 功能色

```css
--success-green: #10b981; /* 成功/在线 */
--warning-yellow: #f59e0b; /* 警告 */
--error-red: #ef4444;      /* 错误 */
--info-purple: #8b5cf6;    /* 信息/智能体 */
```

#### 中性色

```css
--gray-50: #f9fafb;   /* 背景色 */
--gray-100: #f3f4f6;  /* 次级背景 */
--gray-200: #e5e7eb;  /* 边框 */
--gray-600: #4b5563;  /* 次要文字 */
--gray-900: #111827;  /* 主要文字 */
```

### 2.2 渐变规范（允许的既定渐变）

#### 导航栏/侧边栏背景

```css
background: linear-gradient(180deg, #111827 0%, #1f2937 100%);
```

#### 功能卡片渐变

```css
blue:   linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
purple: linear-gradient(135deg, #8b5cf6 0%, #7c3aed 100%);
green:  linear-gradient(135deg, #10b981 0%, #059669 100%);
```

### 2.3 功能模块颜色映射（必须遵循）

| 功能模块 | 主色      | 辅色      |
| ---- | ------- | ------- |
| 脱敏沙箱 | #3b82f6 | #2563eb |
| 工作流  | #8b5cf6 | #7c3aed |
| 智能体  | #10b981 | #059669 |
| 知识库  | #f59e0b | #d97706 |

### 2.4 高亮标注（脱敏沙箱文档区）

- 人名高亮：`background: #fef3c7`（黄色）
- 账号高亮：`background: #dbeafe`（蓝色）
- 金额高亮：`background: #fed7aa`（橙色）

***

## 3. 字体与排版

### 3.1 字体栈（必须统一）

```css
font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI',
             'Roboto', 'Helvetica Neue', Arial, sans-serif;
```

### 3.2 字号与行高（必须统一）

| 用途   | 大小              | 权重             | 行高  |
| ---- | --------------- | -------------- | --- |
| 页面标题 | 24px (1.5rem)   | bold (700)     | 1.2 |
| 区块标题 | 20px (1.25rem)  | semibold (600) | 1.3 |
| 卡片标题 | 18px (1.125rem) | semibold (600) | 1.4 |
| 正文   | 14px (0.875rem) | normal (400)   | 1.5 |
| 辅助文字 | 12px (0.75rem)  | normal (400)   | 1.4 |
| 小号文字 | 10px (0.625rem) | normal (400)   | 1.3 |

### 3.3 文本样式要点（必须遵循）

- 主文本使用 `--gray-900`；辅助信息使用 `--gray-600`。
- 表格表头使用小号大写风格（详见“表格规范”）。

***

## 4. 间距与布局度量

### 4.1 间距系统（基于 4px 基准单位）

```
4px  (0.25rem) - 极小间距
8px  (0.5rem)  - 小间距
12px (0.75rem) - 中小间距
16px (1rem)    - 标准间距
24px (1.5rem)  - 大间距
32px (2rem)    - 超大间距
48px (3rem)    - 区块间距
```

### 4.2 圆角规范（必须遵循）

```css
rounded-sm: 0.125rem;  /* 2px - 小元素 */
rounded: 0.25rem;      /* 4px - 按钮/输入框 */
rounded-lg: 0.5rem;    /* 8px - 卡片 */
rounded-xl: 0.75rem;   /* 12px - 大卡片 */
rounded-2xl: 1rem;     /* 16px - 气泡 */
rounded-full: 9999px;  /* 圆形 - 头像/徽章 */
```

### 4.3 阴影系统（必须遵循）

```css
shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.05);
shadow:    0 1px 3px rgba(0, 0, 0, 0.1);
shadow-md: 0 4px 6px rgba(0, 0, 0, 0.1);
shadow-lg: 0 10px 15px rgba(0, 0, 0, 0.1);
shadow-xl: 0 20px 25px rgba(0, 0, 0, 0.1);
hover:shadow-xl: 0 20px 25px rgba(0, 0, 0, 0.15);
```

***

## 5. 动效标准

### 5.1 过渡时长（必须遵循）

```css
transition-fast: 150ms; /* 快速反馈 */
transition-base: 200ms; /* 标准过渡 */
transition-slow: 300ms; /* 慢速动画 */
```

### 5.2 缓动函数（必须遵循）

```css
ease-in-out: cubic-bezier(0.4, 0, 0.2, 1);
ease-out:    cubic-bezier(0, 0, 0.2, 1);
ease-in:     cubic-bezier(0.4, 0, 1, 1);
```

### 5.3 既定动效模式（必须实现）

- 卡片悬浮：`transition: all 0.3s ease;` + `transform: translateY(-2px);` + 阴影增强（Feature Card / Plugin Card）。
- 处理中提示：
  - 脱敏进度条：高度 8px，300ms ease。
  - 对话“正在输入”：3 个跳动圆点，延迟 0.2s / 0.4s。
  - 知识库“处理中”：RefreshCw 图标旋转（实现需保持一致）。

***

## 6. 应用结构与页面布局

### 6.1 总体结构（必须遵循）

- 布局：经典“侧边栏 + 主内容区”。
- 侧边栏固定宽度：256px；高度：100vh。
- 顶部栏固定高度：64px。

### 6.2 侧边栏（Sidebar）

#### 结构（必须包含）

- Logo 区域：图标徽章 + 产品名称与 Slogan
- 导航菜单：菜单项列表
- 底部用户信息：头像 + 邮箱 + 版本

#### 样式（必须遵循）

```css
background: linear-gradient(180deg, #111827 0%, #1f2937 100%);
border-bottom: 1px solid rgba(255, 255, 255, 0.1);
```

### 6.3 顶部栏（Top Bar）

- 背景：#ffffff
- 底部分割线：`border-bottom: 1px solid #e5e7eb;`
- 内边距：`padding: 0 32px;`
- 内容：左侧“状态指示器 + 系统信息”，右侧“快捷操作按钮”

### 6.4 主内容区（Main Content）

#### 模式 1：标准滚动布局（大部分页面）

```css
overflow-y: auto;
padding: 32px;
```

#### 模式 2：全屏填充布局（脱敏沙箱、对话应用）

```css
height: 100%;
overflow: hidden;
```

***

## 7. 组件规范（通用）

### 7.1 导航菜单项（Nav Item）

#### 视觉状态（必须遵循）

未选中：

- 背景：transparent
- 文字：#d1d5db（gray-300）
- 悬浮：`background: rgba(255, 255, 255, 0.05)`

选中：

- 背景：#3b82f6
- 文字：#ffffff
- 圆角：8px

#### 尺寸（必须遵循）

- 高度：48px
- 内边距：12px 16px
- 图标：20px × 20px
- 文本：14px

#### 结构（参考）

```html
<button class="nav-item [状态类]">
  <div class="flex items-center space-x-3">
    <icon /> <!-- 20px × 20px -->
    <span>菜单名称</span>
  </div>
  <badge /> <!-- 可选徽章 -->
</button>
```

### 7.2 统计卡片（Stat Card）

#### 容器样式（必须遵循）

```css
background: white;
border: 1px solid #e5e7eb;
border-radius: 8px;
padding: 20px;
box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
```

#### 数值与标签（必须遵循）

```css
.value { font-size: 24px; font-weight: 700; color: #111827; }
.label { font-size: 14px; color: #6b7280; }
```

### 7.3 功能入口卡片（Feature Card）

- 背景：使用“渐变规范”的 blue / purple / green 三类之一
- 悬浮：0.3s 过渡 + 阴影增强 + 上移 2px

### 7.4 按钮（Button）

#### 主要按钮（Primary）

```css
background: #3b82f6;
color: white;
padding: 10px 24px;
border-radius: 8px;
font-size: 14px;
font-weight: 500;
hover { background: #2563eb; }
disabled { opacity: 0.5; cursor: not-allowed; }
```

#### 次要按钮（Secondary）

```css
background: transparent;
color: #6b7280;
border: 1px solid #d1d5db;
padding: 10px 24px;
border-radius: 8px;
hover { background: #f3f4f6; }
```

#### 图标按钮（Icon Button）

```css
width: 40px;
height: 40px;
border-radius: 8px;
padding: 8px;
hover { background: #f3f4f6; }
```

### 7.5 输入控件（Input）

#### 文本输入框（必须遵循）

```css
width: 100%;
padding: 10px 16px;
border: 1px solid #d1d5db;
border-radius: 8px;
font-size: 14px;
color: #111827;
focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}
```

#### 多行文本框（必须遵循）

```css
resize: none;
min-height: 96px;
line-height: 1.5;
```

#### 下拉选择框（必须遵循）

```css
appearance: none;
padding-right: 32px;
background-image: url("data:image/svg+xml,...");
```

### 7.6 徽章（Badge）

#### 状态徽章（必须遵循）

```css
/* 成功 */ background: #d1fae5; color: #065f46;
/* 警告 */ background: #fef3c7; color: #92400e;
/* 错误 */ background: #fee2e2; color: #991b1b;
/* 信息 */ background: #dbeafe; color: #1e40af;
```

#### 功能徽章（如“核心”标签）

```css
background: #3b82f6;
color: white;
padding: 2px 8px;
border-radius: 9999px;
font-size: 10px;
font-weight: 500;
```

### 7.7 表格（Table）

#### 表头（必须遵循）

```css
th {
  padding: 12px 24px;
  text-align: left;
  font-size: 11px;
  font-weight: 600;
  color: #6b7280;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
```

#### 单元格（必须遵循）

```css
td {
  padding: 16px 24px;
  font-size: 14px;
  color: #111827;
}
tr:hover { background: #f9fafb; }
```

***

## 8. 业务页面模板（关键界面）

### 8.1 工作台（Dashboard）

- 标题：“欢迎使用 CheersAI 👋”
- 副标题：“安全可控的AI协作平台”
- 统计卡片：4 个
- 功能入口卡片：脱敏沙箱（蓝）、工作流编排（紫）、AI 智能体（绿）
- 最近活动：显示最近 4 条，包含相对时间、类型、关联对象、状态图标（绿色勾）
- 系统健康度/模型状态卡片：显示 CPU/内存与在线状态

### 8.2 脱敏沙箱（核心）

#### 布局（必须遵循）

- 三栏固定布局：左 300px / 中自适应 / 右 280px

#### 左栏组件（300px）

1. 文件上传区（必须遵循）

```css
尺寸: 300px × 200px
边框: 2px dashed #cbd5e1
背景: white
圆角: 8px
hover: border-color #3b82f6; background rgba(59, 130, 246, 0.05)
```

内容：Upload 图标 48px；主文案 14px bold；副文案 12px；格式提示 12px gray；文件占位图标 64px

1. 脱敏策略选择（必须遵循）

```css
宽度: 100%
高度: 52px
边框: 1px solid #e5e7eb
圆角: 8px
padding: 12px
hover: background #f9fafb
```

策略：掩码 / 替换 / 加密（FPE）/ 打乱

1. 警告按钮（必须遵循）

```css
背景: #fef3c7
边框: #fcd34d
图标: AlertTriangle
```

1. 主操作按钮（必须遵循）

```css
宽度: 100%
高度: 48px
背景: #3b82f6 渐变
图标: Play (20px)
阴影: 0 4px 6px rgba(59, 130, 246, 0.25)
```

#### 中栏组件（自适应）

- 标签页：原始文档 / 脱敏文档 / 还原结果

```css
选中: color #3b82f6; border-bottom 2px solid #3b82f6; background #eff6ff
未选中: color #6b7280; hover background #f9fafb
```

- 处理进度条（处理中显示）

```css
高度: 8px
背景: #e5e7eb
进度条: #3b82f6
圆角: 9999px
动画: 300ms ease
```

- 摘要区域

```css
背景: #f9fafb
边框: 1px solid #e5e7eb
圆角: 8px
padding: 16px
字体: 14px
```

#### 右栏组件（280px）

- 标题：“审计日志”（16px semibold）+ 导出按钮（Download + “导出”）
- 日志条目（必须遵循）

```css
margin-bottom: 12px;
font-size: 13px;
```

```css
.log-header { color: #111827; font-weight: 500; }
.log-detail { color: #6b7280; margin-left: 16px; margin-top: 4px; }
```

#### 底部 WORM 审计条（必须遵循）

```css
位置: 固定底部
高度: 48px
背景: linear-gradient(90deg, #1f2937 0%, #111827 100%)
文字: white
padding: 0 16px
```

### 8.3 对话应用（Chatbot）

#### 左侧对话列表（264px）

- 新建对话按钮（必须遵循）：高度 40px，背景 #3b82f6，文字 14px medium，图标 16px，圆角 8px，底部间距 16px
- 对话项（必须遵循）

```css
padding: 12px;
border-radius: 8px;
border: 1px solid transparent;
选中: background #eff6ff; border-color #bfdbfe;
未选中: hover background #f9fafb;
```

#### 右侧对话区域

- 消息气泡（必须遵循）

```css
AI:   左对齐; max-width 768px; background white; border 1px solid #e5e7eb; color #111827; border-radius 16px; padding 12px 16px;
用户: 右对齐; max-width 768px; background #3b82f6; color white; border-radius 16px; padding 12px 16px;
```

- 头像（必须遵循）：32×32，圆角 8px；AI 头像为紫蓝渐变 + Bot 图标；用户头像为蓝色 + “用”
- 隐私保护标识：Shield 图标（绿色）+ “已自动脱敏保护”
- 输入区多行输入框（必须遵循）

```css
min-height: 96px;
background: #f9fafb;
border: 1px solid #e5e7eb;
border-radius: 16px;
padding: 12px 16px;
resize: none;
focus-within: ring 2px #3b82f6;
```

- 发送按钮：48×48，背景 #3b82f6，圆角 12px，Send 图标 20px；禁用 opacity 0.5
- 快捷操作胶囊按钮（3 个）：`padding 4px 12px; background #f3f4f6; radius 9999px; font 12px; hover #e5e7eb`

### 8.4 知识库（Knowledge Base）

- 顶部：标题 + “导入文档”按钮
- 搜索与筛选栏：`[搜索框(flex-1)] [分类下拉] [状态下拉]`
- 搜索框要求：左内边距 40px；Search 图标 20px 绝对定位；placeholder “搜索知识库内容...”
- 表格列宽与对齐（必须遵循）
  - 文档名称：自适应，左对齐（图标+文件名）
  - 分类：100px，左对齐（徽章）
  - 大小：100px，左对齐
  - 向量数：100px，左对齐
  - 状态：120px，左对齐（徽章+图标）
  - 更新时间：120px，左对齐（相对时间）
  - 操作：120px，右对齐（3 个图标按钮）

### 8.5 模型管理（Models Management）

- 添加模型面板（必须遵循）：背景 #eff6ff；边框 #bfdbfe；圆角 8px；padding 24px；2 列网格表单
- Logo（必须遵循）：56×56；蓝色渐变；圆角 12px；首字母白色居中
- 状态徽章（必须遵循）：背景 #d1fae5；文字 #065f46；圆点 8px 绿色脉冲
- 24 小时趋势图（必须遵循）：高度 64px；24 个柱；柱背景 #3b82f6；hover #2563eb；柱圆角 2px 2px 0 0

### 8.6 审计日志（Audit Log）

- 筛选栏：搜索 / 类型 / 时间范围 / 状态（4 列网格）
- 类型徽章颜色（必须遵循）

```css
脱敏操作: #dbeafe / #1e40af
模型调用: #e9d5ff / #7c3aed
数据还原: #fef3c7 / #92400e
系统配置: #d1fae5 / #065f46
```

- 表格行（必须遵循）：`hover background #f9fafb; transition 200ms; padding 16px 24px;`
- WORM 提示条（必须遵循）：固定底部；高度 80px；背景 `linear-gradient(90deg, #1f2937 0%, #111827 100%)`；文字 white；padding 16px；圆角 8px

### 8.7 插件市场（Plugins Marketplace）

- 插件卡片（必须遵循）：白底、1px 边框、圆角 8px、padding 20px、0.3s 过渡
- hover：阴影增强 + 上移 2px
- 图标盒（必须遵循）：48×48；紫蓝渐变；圆角 12px；白字居中

### 8.8 Prompt 工程（Prompt Engineering）

- 编辑区文本框：高度 192px；圆角 8px；padding 12px；14px；focus 蓝色边框 + 轻量 ring
- 输出预览区：高度 192px；背景 #f9fafb；边框 #e5e7eb；圆角 8px；padding 12px；14px；文字 #6b7280；可滚动

### 8.9 系统设置（Settings）

- 设置项（必须遵循）：左右布局；纵向间距 12px；分割线 #f3f4f6
- 标题：14px、500、#111827；描述：12px、#6b7280，顶部间距 4px
- 底部按钮组：`flex justify-end space-x-3; margin-top 24px;`（恢复默认=次要按钮，保存设置=主要按钮）

***

## 9. 交互流程（关键路径）

### 9.1 文件上传流程（必须遵循）

```
用户点击上传区域 → 调用 openFile → 返回路径数组 → 显示文件信息 → 解析内容 → 识别敏感信息 → 高亮预览
```

### 9.2 脱敏处理流程（必须遵循）

```
上传文档 → 解析 → PII识别 → 原始文档高亮 → 点击开始脱敏 → 执行策略/生成映射表/记录审计 → 脱敏文档展示 → 发送模型 → 接收响应 → 数据还原 → 还原结果展示 → 记录完整审计
```

### 9.3 对话流程（必须遵循）

```
输入消息 → 检测敏感信息（有则提示）→ 本地脱敏 → 发送模型 → 显示正在输入动画 → 接收响应 → 还原敏感数据 → 渲染到对话区 → 记录审计
```

### 9.4 知识库导入流程（必须遵循）

```
点击导入 → 选择文件（批量）→ 开始处理（解析/提取/分块/向量化）→ 存储向量库 → 更新列表 → 显示处理中（旋转）→ 完成后变更为已完成
```

***

## 10. 图标规范

### 10.1 图标库（必须统一）

- 全部使用 Lucide Icons。

### 10.2 图标索引（既定）

- home（首页）
- shield（安全/脱敏）
- database（数据库）
- bot（AI 智能体）
- workflow（工作流）
- server（服务器/模型）
- file-text（文件/文档）
- settings（设置）
- history（历史/审计）
- upload（上传）
- download（下载）
- lock（锁定/加密）
- eye（查看）
- alert-triangle（警告）

### 10.3 图标尺寸（必须遵循）

- 导航菜单项：20px
- 对话“新建”按钮：16px
- 上传区主图标：48px
- 文件占位图标：64px

***

## 11. 适配规则（Desktop）

### 11.1 布局适配（必须遵循）

- 侧边栏固定 256px；主内容区自适应宽度。
- 脱敏沙箱三栏固定：300px / 自适应 / 280px。
- 标准页面使用“主内容区滚动”；脱敏沙箱与对话应用使用“全屏填充 + 内部滚动区域”。

### 11.2 字体与密度（必须遵循）

- 正文基准 14px；全局间距以 4px 体系为准，禁止引入非体系随机间距。

***

## 12. 可访问性（A11y）要求

### 12.1 键盘可用性（必须满足）

- 所有可点击元素必须可通过键盘聚焦与触发（Tab/Enter/Space）。
- 焦点状态必须可见：输入控件采用既定 focus ring（3px rgba(59,130,246,0.1) 或 ring 2px #3b82f6）。

### 12.2 视觉可读性（必须满足）

- 主文本与背景对比必须清晰（主文本 #111827；辅助文本 #4b5563/#6b7280；浅背景 #f9fafb/#ffffff）。
- 状态信息不得只依赖颜色：应搭配图标/文本（如“已完成 + CheckCircle”）。

### 12.3 动效可控（必须满足）

- “正在输入/处理中”动效必须不影响主要内容阅读与操作。

***

## 13. 组件用法与实现约束（开发侧）

### 13.1 Tailwind 使用原则（必须遵循）

1. 优先使用 Tailwind 内置类。
2. 自定义样式统一写在 `styles.css`。
3. 组件特定样式可用内联样式（仅限必要场景）。

### 13.2 CSS 命名约定（必须遵循）

```css
.component-name { }
.is-active { }
.is-disabled { }
.component-name--variant { }
```

***

## 14. 验收指标（UI 一致性与可用性）

### 14.1 一致性验收（必须通过）

- 色彩：全局仅使用本规范定义的主色/功能色/中性色与既定渐变；模块配色遵循“功能模块颜色映射”。
- 排版：标题/正文/辅助文字严格遵循字号表；表格表头/单元格遵循既定字号与 padding。
- 间距：所有布局间距来源于 4px 体系；主内容区 padding 固定 32px（标准页面）。
- 圆角/阴影：卡片 8px 圆角 + 既定阴影；气泡 16px 圆角；图标按钮 8px 圆角。
- 状态：hover/active/selected/disabled 状态完整且视觉区分明确。

### 14.2 可用性验收（必须通过）

- 关键流程具备完整反馈：上传、脱敏、模型调用、还原、导入、处理中等状态可见且可理解。
- 列表/表格可扫读：列对齐与列宽符合规范；行 hover 反馈一致。
- 可访问性：键盘可操作、焦点可见、状态不只依赖颜色。

***

## 15. 双工作区总体架构（新增）

### 15.1 架构边界（必须遵循）

- `Vault` 功能区必须支持完全离线本地操作；其页面资源、依赖、字体、图标、WASM 与本地数据能力必须内嵌到客户端包体。
- `Desktop` 功能区必须依赖网络连接访问 Web 系统；其内容承载在同一主窗口内的在线工作区容器中。
- `Vault` 与 `Desktop` 必须共享同一套 Design Tokens、排版、间距、圆角、阴影、反馈节奏与状态语义。
- `Vault ↔ Desktop` 必须在单窗口中完成切换，禁止弹出新顶层窗口。

### 15.2 推荐信息架构（必须遵循）

- 全局只保留一套主导航，不允许同屏出现两套同级左侧主菜单。
- 顶部工作区条负责 `Vault / Desktop` 一级工作区切换。
- 左侧侧边栏负责当前工作区的一级功能导航。
- `Desktop` 在嵌入态下隐藏其原生左侧主菜单，改由宿主统一导航承载。

### 15.2A 产品命名与窗体标题（必须遵循）

- 最终打包产品名称统一为：`CheersAI Desktop`。
- 应用窗体左上角品牌名称统一显示：`CheersAI Desktop`，不得显示为 `CheersAI Vault`。
- 应用窗体 Title 统一为：`CheersAI Desktop · 智享AI，安全随行`。
- 所有安装包名称、窗口标题、关于页、欢迎页、Dock/任务栏显示名、更新弹窗名称必须保持一致。
- `Vault` 仅作为离线工作区名称，不作为整个客户端产品名称使用。
- `在线客服` 按钮必须保留在顶层可见操作区，不得在单窗口改造中删除。

### 15.3 全局导航架构（推荐）

```text
+----------------------------------------------------------------------------------+
| Traffic Light | CheersAI Desktop | 智享AI，安全随行 | 网络状态 | 同步状态 | 用户 | 在线客服 | 设置 | 帮助 |
+----------------------+-----------------------------------------------------------+
| 全局左栏 256px       | 顶部工作区条 64px                                         |
|----------------------|-----------------------------------------------------------|
| CheersAI Desktop     | [Vault] [Desktop]                     Cmd+1 / Cmd+2        |
|                      |-----------------------------------------------------------|
| 文件脱敏             | 当前工作区内容区                                          |
| 文件反脱敏           |                                                           |
| 文件管理             | Vault: 本地页面 / Desktop: 在线工作区                     |
| FileBay 设置         |                                                           |
| 规则配置             |                                                           |
| 沙箱管理             |                                                           |
| 操作日志             |                                                           |
+----------------------+-----------------------------------------------------------+
| 状态托盘：离线可用 / 待同步队列 / 更新状态 / 版本信息                            |
+----------------------------------------------------------------------------------+
```

### 15.4 快捷键（推荐）

- `Cmd/Ctrl + 1`：切换到 `Vault`
- `Cmd/Ctrl + 2`：切换到 `Desktop`
- `Cmd/Ctrl + Shift + S`：打开同步面板
- `Cmd/Ctrl + /`：打开全局快捷帮助
- `Esc`：从 `Desktop` 子工作区焦点退回宿主主壳

***

## 16. 工作区布局与菜单规范（新增）

### 16.1 单窗口布局（必须遵循）

- 整体仍遵循“侧边栏 + 顶部栏 + 主内容区”。
- 侧边栏固定宽度：256px。
- 顶部栏固定高度：64px。
- 工作区条固定高度：48px，可并入顶部栏下方。
- 主内容区在 `Vault` 模式下为本地 React 页面；在 `Desktop` 模式下为在线工作区容器。

### 16.2 菜单分层（必须遵循）

- 一级：工作区切换（`Vault / Desktop`）
- 二级：当前工作区功能菜单
- 三级：当前页面局部筛选、标签、排序与搜索
- 不允许一级与二级同时由两个不同左侧菜单承担。

### 16.3 `Vault` 菜单规范（推荐）

- 菜单项：文件脱敏、文件反脱敏、文件管理、FileBay 设置、规则配置、沙箱管理、操作日志。
- 选中态：主蓝色实底、白字、8px 圆角。
- 非选中态：透明背景、灰 300 文字、悬浮态白色 5% 蒙层。

### 16.4 `Desktop` 嵌入态菜单规范（推荐）

- 宿主壳统一承载一级模块导航。
- `Desktop` 原一级菜单迁移为顶部模块标签或宿主统一菜单树。
- 推荐模块：我的 Agent、对话、知识库、工具插件、工作流、应用中心、探索、审计日志。

### 16.5 工作区切换热区（必须包含）

- 顶部工作区条中的 `Vault / Desktop` 双标签。
- 左侧导航中 `CheersAI` 作为 `Desktop` 工作区入口时，需与顶部工作区条联动高亮。
- 切换过程中禁止整窗刷新或白屏。

### 16.6 顶部固定操作项（必须包含）

- 顶部左上角固定展示产品名：`CheersAI Desktop`。
- 顶部标题区固定展示 slogan：`智享AI，安全随行`。
- 顶部右侧操作区必须保留：`在线客服`、用户信息、设置/帮助入口。
- `在线客服` 在 `Vault` 与 `Desktop` 两个工作区中都需保持可见或可一步直达。

***

## 17. 离线 / 在线状态指示器规范（新增）

### 17.1 状态类型

- 离线可用（Vault）
- 连接中（Desktop）
- 在线正常
- 弱网高延迟
- 断网不可用
- 同步排队
- 同步冲突

### 17.2 状态指示器表（必须统一）

| 状态          | 颜色        | 文案                   | 位置             | 自动消失时机  | 无障碍朗读文本               |
| ----------- | --------- | -------------------- | -------------- | ------- | --------------------- |
| Vault 离线可用  | `#10b981` | 离线可用                 | 顶部栏左侧          | 常驻      | Vault 工作区当前离线可用       |
| Desktop 连接中 | `#f59e0b` | 正在连接 Desktop...      | 顶部栏中央          | 成功后 2s  | Desktop 工作区正在连接       |
| Desktop 弱网  | `#f59e0b` | 网络较慢，部分内容延迟          | 顶部栏中央          | 恢复后 3s  | 网络较慢，Desktop 部分内容延迟   |
| Desktop 断网  | `#ef4444` | Desktop 不可用，请检查网络并重试 | 顶部栏 + 内容区内联错误条 | 不自动消失   | Desktop 当前不可用，请检查网络连接 |
| 同步排队        | `#8b5cf6` | 3 项待同步               | 顶部栏右侧          | 队列清空后消失 | 当前有 3 项待同步任务          |
| 同步冲突        | `#ef4444` | 发现同步冲突，请处理           | 顶部栏右侧 + 冲突抽屉   | 用户处理后消失 | 发现同步冲突，请选择处理方式        |

### 17.3 状态交互原则（必须遵循）

- 状态提示必须使用颜色 + 图标 + 文案三重表达，不得仅依赖颜色。
- `Desktop` 局部失败时，`Vault` 其他区域必须保持可操作。
- 网络恢复后自动重试最多 1 次，再显示显式 `重试` 按钮。

***

## 18. 加载、空状态、错误状态文案与插画规范（新增）

### 18.1 加载态（必须遵循）

- 优先使用 Skeleton，而非全屏 Spinner。
- Skeleton 必须模拟真实布局，避免加载完成时大幅跳动。
- 局部加载不阻断其他区域交互。

### 18.2 错误态文案原则（必须遵循）

- 说明发生了什么。
- 提示下一步如何恢复。
- 避免技术术语直出给终端用户。
- 不归咎于用户。

### 18.3 错误态文案库（推荐）

- `Desktop 连接失败`：当前无法连接 Desktop，请检查网络后重试。
- `加载超时`：当前内容加载超时，请点击重试。
- `同步冲突`：检测到本地与云端内容不一致，请选择保留版本。
- `离线限制`：当前处于离线状态，Desktop 功能暂不可用；你仍可继续使用 Vault。

### 18.4 空状态文案库（推荐）

- `Vault 零数据`：当前还没有本地资产，立即导入第一份文件开始处理。
- `Desktop 零结果`：没有找到匹配结果，请调整筛选条件或搜索词。
- `待同步为空`：当前没有待同步任务。

### 18.5 插画资产命名（推荐）

- `offline-vault-empty-light.svg`
- `offline-vault-empty-dark.svg`
- `desktop-network-error-light.svg`
- `desktop-network-error-dark.svg`
- `sync-conflict-light.svg`
- `sync-conflict-dark.svg`

***

## 19. 设计 Tokens 工程映射（新增）

### 19.1 CSS 自定义属性映射（必须提供）

```css
:root {
  --primary-blue: #3b82f6;
  --primary-blue-dark: #2563eb;
  --primary-blue-light: #60a5fa;
  --success-green: #10b981;
  --warning-yellow: #f59e0b;
  --error-red: #ef4444;
  --info-purple: #8b5cf6;
  --gray-50: #f9fafb;
  --gray-100: #f3f4f6;
  --gray-200: #e5e7eb;
  --gray-600: #4b5563;
  --gray-900: #111827;
  --radius-sm: 0.125rem;
  --radius-md: 0.25rem;
  --radius-lg: 0.5rem;
  --radius-xl: 0.75rem;
  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.05);
  --shadow-md: 0 4px 6px rgba(0, 0, 0, 0.1);
  --shadow-lg: 0 10px 15px rgba(0, 0, 0, 0.1);
  --transition-fast: 150ms;
  --transition-base: 200ms;
  --transition-slow: 300ms;
  --transition-workspace: 250ms;
}
```

### 19.2 Tailwind 配置映射（必须提供）

```ts
export default {
  theme: {
    extend: {
      colors: {
        primary: {
          DEFAULT: 'var(--primary-blue)',
          dark: 'var(--primary-blue-dark)',
          light: 'var(--primary-blue-light)',
        },
        success: { DEFAULT: 'var(--success-green)' },
        warning: { DEFAULT: 'var(--warning-yellow)' },
        error: { DEFAULT: 'var(--error-red)' },
        info: { DEFAULT: 'var(--info-purple)' },
        gray: {
          50: 'var(--gray-50)',
          100: 'var(--gray-100)',
          200: 'var(--gray-200)',
          600: 'var(--gray-600)',
          900: 'var(--gray-900)',
        },
      },
      borderRadius: {
        sm: '0.125rem',
        DEFAULT: '0.25rem',
        lg: '0.5rem',
        xl: '0.75rem',
      },
      boxShadow: {
        sm: '0 1px 2px rgba(0,0,0,0.05)',
        md: '0 4px 6px rgba(0,0,0,0.1)',
        lg: '0 10px 15px rgba(0,0,0,0.1)',
      },
      transitionDuration: {
        150: '150ms',
        200: '200ms',
        250: '250ms',
        300: '300ms',
      },
    },
  },
}
```

### 19.3 Flutter ThemeData 映射（推荐）

```dart
final cheersAiTheme = ThemeData(
  fontFamily: '-apple-system',
  colorScheme: const ColorScheme.light(
    primary: Color(0xFF3B82F6),
    primaryContainer: Color(0xFF2563EB),
    secondary: Color(0xFF10B981),
    tertiary: Color(0xFFF59E0B),
    error: Color(0xFFEF4444),
    surface: Color(0xFFFFFFFF),
    onSurface: Color(0xFF111827),
  ),
);
```

### 19.4 Less / Sass 变量映射（推荐）

```scss
$primary-blue: #3b82f6;
$primary-blue-dark: #2563eb;
$success-green: #10b981;
$warning-yellow: #f59e0b;
$error-red: #ef4444;
$info-purple: #8b5cf6;
$gray-50: #f9fafb;
$gray-900: #111827;
$radius-lg: 0.5rem;
$shadow-md: 0 4px 6px rgba(0, 0, 0, 0.1);
$transition-base: 200ms;
```

***

## 20. 响应式断点与网格系统（新增）

| 断点 | 视口          | 栅格列数 | 槽宽   | 外边距  | 组件最大宽度 |
| -- | ----------- | ---- | ---- | ---- | ------ |
| XL | `>=1366`    | 12   | 24px | 32px | 1440px |
| LG | `1024~1365` | 12   | 20px | 24px | 1200px |
| MD | `768~1023`  | 8    | 16px | 20px | 960px  |
| SM | `<=767`     | 4    | 12px | 16px | 100%   |

### 20.1 响应式策略（必须遵循）

- `Vault` 左侧栏在桌面端保留固定 256px，在中等宽度下允许折叠为 64px。
- `Desktop` 嵌入态一级导航不得再以第二套左栏出现。
- 顶部工作区条与状态指示器在窄屏下允许合并为两行。

***

## 21. 单窗口切换动效规范（新增）

### 21.1 动效目标

- `Vault ↔ Desktop` 切换动画时长：250ms。
- 仅允许使用 `opacity + transform` 组合，优先走 GPU 加速。
- 目标帧率：60fps。
- 切换过程内存波动目标：≤30MB。

### 21.2 动效实现（推荐）

```css
.workspace-enter {
  opacity: 0;
  transform: translateY(8px) scale(0.98);
}

.workspace-enter-active {
  opacity: 1;
  transform: translateY(0) scale(1);
  transition: opacity 250ms cubic-bezier(0.4, 0, 0.2, 1),
              transform 250ms cubic-bezier(0.4, 0, 0.2, 1);
}

.workspace-exit {
  opacity: 1;
}

.workspace-exit-active {
  opacity: 0;
  transition: opacity 180ms cubic-bezier(0.4, 0, 0.2, 1);
}
```

### 21.3 切换约束（必须遵循）

- 切换过程中不得出现整窗白屏。
- 切换过程中不得丢失 `Vault` 本地上下文。
- `Desktop` 断网时从 `Desktop` 回切 `Vault` 必须立即成功。

***

## 22. 无障碍补充要求（新增）

### 22.1 键盘顺序（必须满足）

- 顶部工作区条
- 左侧主导航
- 当前工作区内容区
- 状态指示器与同步操作
- 底部版本与用户信息

### 22.2 ARIA 语义（必须满足）

- 工作区切换：`role="tablist"`
- 工作区按钮：`role="tab"`
- 主导航：`role="navigation"`
- 网络与错误提示：`role="alert"`
- 同步状态：`aria-live="polite"`

### 22.3 高对比度与朗读（必须满足）

- 状态信息必须同时包含颜色、图标、文案。
- 切换工作区时需朗读：
  - `已进入离线 Vault 工作区`
  - `已进入在线 Desktop 工作区`

***

## 23. 新增验收指标（补充）

### 23.1 双工作区体验验收

- 单窗口切换成功率：≥99%
- 首次点击工作区切换成功率：≥95%
- 菜单层级误判率：≤3%
- 内容区有效宽度提升：≥18%
- 菜单操作步长降低：≥30%

### 23.2 网络与恢复验收

- 无网络进入 `Vault` 成功率：100%
- 网络恢复后 `Desktop` 重连成功率：≥95%
- 重试按钮单次成功率：≥90%
- 同步冲突提示出现时机正确率：100%

