# CheersAI UI 组件库使用指南

## 📚 概述

本组件库提供符合 **CheersAI 产品 UI 规范 v1.0** 的标准化 UI 组件，确保全局视觉一致性和交互体验。

---

## 🎨 设计原则

### 色彩系统
- **主色**: #3b82f6（品牌蓝）
- **功能色**: 成功(绿)、警告(黄)、错误(红)、信息(紫)
- **中性色**: 灰度系统 50-900

### 间距系统
- 基于 **4px** 基准单位
- 标准间距：4px, 8px, 12px, 16px, 24px, 32px

### 圆角系统
- 按钮/输入框：**8px**
- 卡片：**8px-12px**
- 徽章：**full**（完全圆角）

### 过渡时长
- 快速：**150ms**
- 标准：**200ms**
- 慢速：**300ms**

---

## 📦 组件列表

### 1. Button（按钮）

#### 变体
- `primary`: 主按钮（蓝色背景）
- `secondary`: 次按钮（白色背景，灰色边框）
- `icon`: 图标按钮（透明背景）

#### 尺寸
- `sm`: 小号
- `md`: 中号（默认）
- `lg`: 大号

#### 使用示例

```tsx
import { Button } from '@/components/ui/cheersai-ui';
import { Save, Download } from 'lucide-react';

// 主按钮
<Button variant="primary">
  保存配置
</Button>

// 带图标的主按钮
<Button variant="primary" icon={Save}>
  保存配置
</Button>

// 次按钮
<Button variant="secondary">
  取消
</Button>

// 图标按钮
<Button variant="icon" icon={Download} size="sm" />

// 加载状态
<Button variant="primary" loading>
  保存中...
</Button>

// 禁用状态
<Button variant="primary" disabled>
  已禁用
</Button>
```

#### 样式规范
- **主按钮**: 背景 #3b82f6，悬浮 #2563eb，白色文字
- **次按钮**: 白色背景，灰色边框，悬浮灰色背景
- **圆角**: 8px
- **过渡**: 200ms

---

### 2. Badge（徽章）

#### 变体
- `success`: 成功状态（绿色）
- `warning`: 警告状态（黄色）
- `error`: 错误状态（红色）
- `info`: 信息状态（紫色）
- `neutral`: 中性状态（灰色）

#### 使用示例

```tsx
import { Badge } from '@/components/ui/cheersai-ui';

<Badge variant="success">已完成</Badge>
<Badge variant="warning">待处理</Badge>
<Badge variant="error">失败</Badge>
<Badge variant="info">新功能</Badge>
<Badge variant="neutral">3</Badge>
```

#### 样式规范
- **圆角**: full（完全圆角）
- **内边距**: 2px 8px
- **字体**: 12px medium
- **背景**: 功能色 10% 透明度
- **边框**: 功能色 20% 透明度

---

### 3. Card（卡片）

#### 属性
- `hover`: 启用悬浮效果
- `selected`: 选中状态
- `onClick`: 点击事件

#### 使用示例

```tsx
import { Card } from '@/components/ui/cheersai-ui';

// 基础卡片
<Card className="p-6">
  <h3>卡片标题</h3>
  <p>卡片内容</p>
</Card>

// 可悬浮卡片
<Card hover className="p-6">
  <h3>悬浮卡片</h3>
</Card>

// 选中状态卡片
<Card selected className="p-6">
  <h3>已选中</h3>
</Card>

// 可点击卡片
<Card hover onClick={() => console.log('clicked')} className="p-6">
  <h3>点击我</h3>
</Card>
```

#### 样式规范
- **背景**: 白色
- **边框**: 灰色 #e5e7eb
- **圆角**: 8px
- **悬浮**: 阴影增强 + 上移 2px
- **选中**: 蓝色边框 + 蓝色背景（5% 透明度）
- **过渡**: 200ms

---

### 4. StatCard（统计卡片）

#### 使用示例

```tsx
import { StatCard } from '@/components/ui/cheersai-ui';
import { Users, FileText, CheckCircle } from 'lucide-react';

<StatCard
  label="总用户数"
  value="1,234"
  icon={Users}
  trend={{ value: "12%", positive: true }}
/>

<StatCard
  label="处理文件"
  value="567"
  icon={FileText}
/>

<StatCard
  label="成功率"
  value="98.5%"
  icon={CheckCircle}
  trend={{ value: "2.3%", positive: true }}
/>
```

#### 样式规范
- **标签**: 14px，灰色 #6b7280
- **数值**: 24px bold，黑色 #111827
- **图标**: 48px 容器，蓝色背景（10% 透明度）
- **趋势**: 成功绿色/错误红色

---

### 5. Input（输入框）

#### 使用示例

```tsx
import { Input } from '@/components/ui/cheersai-ui';

// 基础输入框
<Input
  placeholder="请输入内容"
/>

// 带标签的输入框
<Input
  label="用户名"
  placeholder="请输入用户名"
/>

// 带帮助文本
<Input
  label="邮箱"
  placeholder="example@email.com"
  helperText="我们不会分享您的邮箱"
/>

// 错误状态
<Input
  label="密码"
  type="password"
  error="密码长度至少 8 位"
/>

// 禁用状态
<Input
  label="只读字段"
  value="不可编辑"
  disabled
/>
```

#### 样式规范
- **边框**: 灰色 #d1d5db
- **圆角**: 8px
- **内边距**: 10px 16px
- **字体**: 14px
- **焦点**: 蓝色边框 + 蓝色 ring（20% 透明度）
- **错误**: 红色边框

---

### 6. Textarea（文本域）

#### 使用示例

```tsx
import { Textarea } from '@/components/ui/cheersai-ui';

<Textarea
  label="描述"
  placeholder="请输入描述"
  rows={4}
/>

<Textarea
  label="备注"
  placeholder="请输入备注"
  helperText="最多 500 字"
  maxLength={500}
/>
```

#### 样式规范
- 与 Input 相同
- **不可调整大小**: resize-none

---

### 7. Message（消息提示）

#### 变体
- `success`: 成功消息
- `warning`: 警告消息
- `error`: 错误消息
- `info`: 信息消息

#### 使用示例

```tsx
import { Message } from '@/components/ui/cheersai-ui';

<Message type="success" title="操作成功">
  配置已成功保存
</Message>

<Message type="warning" title="注意">
  此操作不可撤销，请谨慎操作
</Message>

<Message type="error" title="错误">
  保存失败，请检查网络连接
</Message>

<Message type="info">
  这是一条信息提示
</Message>

// 可关闭的消息
<Message 
  type="success" 
  title="成功"
  onClose={() => console.log('closed')}
>
  操作已完成
</Message>
```

#### 样式规范
- **背景**: 功能色 5-10% 透明度
- **边框**: 功能色 20% 透明度
- **圆角**: 8px
- **内边距**: 16px
- **图标**: 功能色

---

### 8. Loading（加载状态）

#### 尺寸
- `sm`: 小号（16px）
- `md`: 中号（32px）
- `lg`: 大号（48px）

#### 使用示例

```tsx
import { Loading } from '@/components/ui/cheersai-ui';

// 基础加载
<Loading />

// 带文字的加载
<Loading text="加载中..." />

// 小号加载
<Loading size="sm" text="处理中..." />

// 大号加载
<Loading size="lg" text="正在初始化..." />
```

#### 样式规范
- **颜色**: 主蓝色 #3b82f6
- **动画**: 旋转动画
- **文字**: 14px，灰色

---

### 9. EmptyState（空状态）

#### 使用示例

```tsx
import { EmptyState } from '@/components/ui/cheersai-ui';
import { Inbox } from 'lucide-react';

<EmptyState
  icon={Inbox}
  title="暂无数据"
  description="当前没有任何记录，点击下方按钮开始添加"
  action={{
    label: "添加记录",
    onClick: () => console.log('add')
  }}
/>
```

#### 样式规范
- **图标容器**: 64px，灰色背景
- **标题**: 18px semibold
- **描述**: 14px，灰色
- **按钮**: 主按钮样式

---

### 10. Switch（开关）

#### 使用示例

```tsx
import { Switch } from '@/components/ui/cheersai-ui';

const [enabled, setEnabled] = useState(false);

<Switch
  checked={enabled}
  onChange={setEnabled}
  label="启用功能"
  description="开启后将自动同步数据"
/>

// 禁用状态
<Switch
  checked={false}
  onChange={() => {}}
  label="已禁用"
  disabled
/>
```

#### 样式规范
- **宽度**: 44px
- **高度**: 24px
- **圆角**: full
- **开启**: 蓝色背景
- **关闭**: 灰色背景
- **过渡**: 200ms

---

## 🎯 使用最佳实践

### 1. 导入组件

```tsx
// 单个导入
import { Button } from '@/components/ui/cheersai-ui';

// 多个导入
import { Button, Card, Badge, Input } from '@/components/ui/cheersai-ui';
```

### 2. 组合使用

```tsx
import { Card, Badge, Button } from '@/components/ui/cheersai-ui';
import { Settings } from 'lucide-react';

<Card hover className="p-6">
  <div className="flex items-center justify-between mb-4">
    <h3 className="text-lg font-semibold">配置项</h3>
    <Badge variant="success">已启用</Badge>
  </div>
  <p className="text-sm text-gray-600 mb-4">
    这是配置项的描述文字
  </p>
  <Button variant="secondary" icon={Settings} size="sm">
    编辑
  </Button>
</Card>
```

### 3. 响应式设计

```tsx
<div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
  <StatCard label="用户" value="100" />
  <StatCard label="文件" value="200" />
  <StatCard label="任务" value="300" />
</div>
```

### 4. 状态管理

```tsx
const [loading, setLoading] = useState(false);
const [error, setError] = useState<string | null>(null);
const [success, setSuccess] = useState(false);

return (
  <>
    {loading && <Loading text="保存中..." />}
    
    {error && (
      <Message type="error" title="错误" onClose={() => setError(null)}>
        {error}
      </Message>
    )}
    
    {success && (
      <Message type="success" title="成功" onClose={() => setSuccess(false)}>
        操作已完成
      </Message>
    )}
    
    <Button 
      variant="primary" 
      onClick={handleSave}
      loading={loading}
      disabled={loading}
    >
      保存
    </Button>
  </>
);
```

---

## ✅ 验收检查清单

使用组件时，请确保：

- [ ] 使用规范的颜色（primary, success, warning, error, info）
- [ ] 使用规范的圆角（8px 为主）
- [ ] 使用规范的间距（4px 体系）
- [ ] 使用规范的过渡时长（200ms 为主）
- [ ] 状态反馈清晰（颜色 + 图标 + 文案）
- [ ] 键盘可访问（Tab 键可聚焦）
- [ ] 焦点状态可见
- [ ] 禁用状态明确
- [ ] 加载状态可见

---

## 📚 相关文档

- `CheersAI产品UI规范.md` - 完整 UI 规范
- `tailwind.config.js` - Tailwind 配置
- `src/index.css` - 全局样式
- `UI_UPDATE_SUMMARY.md` - UI 更新总结

---

## 🎨 设计 Tokens 快速参考

```css
/* 主色 */
--primary-blue: #3b82f6;
--primary-blue-dark: #2563eb;

/* 功能色 */
--success-green: #10b981;
--warning-yellow: #f59e0b;
--error-red: #ef4444;
--info-purple: #8b5cf6;

/* 中性色 */
--gray-50: #f9fafb;
--gray-100: #f3f4f6;
--gray-200: #e5e7eb;
--gray-600: #4b5563;
--gray-900: #111827;

/* 过渡 */
--transition-fast: 150ms;
--transition-base: 200ms;
--transition-slow: 300ms;
```

---

**版本**: v1.0.0  
**更新时间**: 2026-05-06  
**符合规范**: CheersAI 产品 UI 规范 v1.0
